use std::{error::Error, net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket}, time::{SystemTime, Duration}};

use bevy::prelude::*;
use bevy_replicon::{prelude::*, renet::{ConnectionConfig, transport::{ServerConfig, ServerAuthentication, NetcodeServerTransport, ClientAuthentication, NetcodeClientTransport}, SendType, ServerEvent}, client};
use clap::Parser;
use serde::{Serialize, Deserialize};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ReplicationPlugins))
        .init_resource::<Cli>()
        .init_resource::<InputsCount>()
        .init_resource::<Timmy>()
        .replicate::<Player>()
        .replicate::<Position>()
        .replicate::<PlayerSpawnedComponent>()
        .add_client_event::<PlayerInput>(SendType::ReliableOrdered { resend_time: Duration::from_millis(300) })
        .add_systems(
            Startup,
        (
            cli_system.pipe(system_adapter::unwrap),
            init_system,
        ))
        .add_systems(Update, 
            (
            player_input_system,
            update_input_count_text,
            entity_tracker_system,
        ))
        .add_systems(Update,
            (
                receive_player_input_system,
                server_connection_events_system,
            ).run_if(resource_exists::<RenetServer>())
        )
        .add_systems(Update, 
            (client_tracker_system, client_random_spawn_system).run_if(resource_exists::<RenetClient>())
        )
        .run();
}

const SERVER_ID: u64 = 0;
const PORT: u16 = 5003;
const PROTOCOL_ID: u64 = 0;

#[derive(Component, Deserialize, Serialize)]
pub struct Player(pub u64);

#[derive(Parser, PartialEq, Resource)]
pub enum Cli
{
    Server {
        #[arg(short, long, default_value_t = PORT)]
        port: u16
    },
    Client {
        #[arg(short, long, default_value_t = Ipv4Addr::LOCALHOST.into())]
        ip: IpAddr,

        #[arg(short, long, default_value_t = PORT)]
        port: u16
    }
}

impl Default for Cli
{
    fn default() -> Self {
        Self::parse()
    }
}

// A resource to track the number of entities spawned locally
#[derive(Resource, Default)]
pub struct InputsCount(u64);

// The event that clients will send to the server when it receives input
// This event will spawn the entities on the server
#[derive(Event, Serialize, Deserialize)]
pub enum PlayerInput
{
    None,
    Shoot(Entity),
}

// A dud component that will be attached to the pre-spawned entities
#[derive(Component, Serialize, Deserialize, Default)]
pub struct PlayerSpawnedComponent
{
    random_stuff: [u64; 20],
    other_random_stuff: [u64; 13],
}

#[derive(Component)]
pub struct RandomOtherComponent;

#[derive(Component)]
pub struct RandomComponent;

#[derive(Component, Serialize, Deserialize)]
pub struct Position(pub Vec2);

// Marker component for the text object that tracks spawn counts
#[derive(Component)]
pub struct PlayerSpawnCountText;

#[derive(Resource, Default)]
pub struct Timmy
{
    pub time_left: f32
}

fn player_input_system(
    mut commands: Commands,
    mut input_writer: EventWriter<PlayerInput>,
    input: Res<Input<KeyCode>>
) {
    if !input.just_pressed(KeyCode::Space)
    {
        return;
    }

    let spawned_entity = commands.spawn((PlayerSpawnedComponent::default(), Replication)).id();
    info!("Client: Spawned {spawned_entity:?} From Input");

    input_writer.send(PlayerInput::Shoot(spawned_entity));
}

// Server-side system that receives the events and spawns its own version of the entity
fn receive_player_input_system(
    mut commands: Commands,
    mut input_reader: EventReader<FromClient<PlayerInput>>,
    mut mapping: ResMut<ClientEntityMap>,
    tick: Res<RepliconTick>,
) {
    for FromClient { client_id, event } in &mut input_reader
    {
        if *client_id == SERVER_ID
        {
            continue;
        }

        let PlayerInput::Shoot(client_entity) = event else { continue; };

        let server_entity = commands.spawn((PlayerSpawnedComponent::default(), Replication)).id();

        info!("Server: Spawned {server_entity:?} From Client Event (which spawned {client_entity:?})");

        mapping.insert(*client_id, ClientMapping { tick: *tick, server_entity: server_entity, client_entity: *client_entity });
    }
}

/// Runs on both server and client, adds extra components when a PlayerSpawnedComponent entity is first created/replicated
fn entity_tracker_system(
    mut commands: Commands,
    mut input_count: ResMut<InputsCount>,
    new_entites: Query<Entity, (With<PlayerSpawnedComponent>, Added<Replication>)>
) {
    for entity in &new_entites
    {
        info!("Client: Seen Entity {entity:?} Spawned");
        input_count.0 += 1;

        commands.entity(entity).insert(RandomComponent);
    }
}

/// Client side only function to try and trigger this bug I am experiencing
fn client_tracker_system(
    mut commands: Commands,
    new_entites: Query<Entity, (With<PlayerSpawnedComponent>, Added<Replication>)>
) {
    for entity in &new_entites
    {
        commands.entity(entity).insert(RandomOtherComponent);
    }
}

fn client_random_spawn_system(
    mut commands: Commands,
    mut tim: ResMut<Timmy>,
    time: Res<Time>,
) {
    tim.time_left -= time.delta_seconds();

    if tim.time_left > 0.0
    {
        return;
    }

    tim.time_left = 5.0;
    commands.spawn(TransformBundle::from_transform(Transform::from_translation(Vec3::new(1.0, 3.0, -69.0))));
}

fn update_input_count_text(
    input_count: Res<InputsCount>,
    mut text_query: Query<&mut Text, With<PlayerSpawnCountText>>,
) {
    if !input_count.is_changed()
    {
        return;
    }

    let input_count = input_count.0;
    text_query.single_mut().sections[0].value = format!("{input_count} total");
}

fn init_system(
    mut commands: Commands,
) {
    commands.spawn(Camera2dBundle::default());

    commands.spawn((TextBundle::from_section(
        "0 total", 
        TextStyle { font_size: 30.0, color: Color::WHITE, ..default() }
    ).with_style(Style { 
        align_self: AlignSelf::FlexEnd, justify_self: JustifySelf::Start, flex_direction: FlexDirection::Column, ..default() 
    }), PlayerSpawnCountText));
}

fn cli_system(
    mut commands: Commands,
    cli: Res<Cli>,
    network_channels: Res<NetworkChannels>,
) -> Result<(), Box<dyn Error>> {
    match *cli {
        Cli::Server { port } => {
            info!("Starting a server on port {port}");
            let server_channels_config = network_channels.server_channels();
            let client_channels_config = network_channels.client_channels();

            let server = RenetServer::new(ConnectionConfig {
                server_channels_config,
                client_channels_config,
                ..Default::default()
            });

            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let public_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
            let socket = UdpSocket::bind(public_addr)?;
            let server_config = ServerConfig {
                max_clients: 10,
                protocol_id: PROTOCOL_ID,
                public_addr,
                authentication: ServerAuthentication::Unsecure,
            };
            let transport = NetcodeServerTransport::new(current_time, server_config, socket)?;

            commands.insert_resource(server);
            commands.insert_resource(transport);

            commands.spawn(TextBundle::from_section(
                "Server",
                TextStyle {
                    font_size: 30.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            commands.spawn((Player(SERVER_ID), Position(Vec2::ZERO)));
        }
        Cli::Client { port, ip } => {
            info!("Starting a client connecting to: {ip:?}:{port}");
            let server_channels_config = network_channels.server_channels();
            let client_channels_config = network_channels.client_channels();

            let client = RenetClient::new(ConnectionConfig {
                server_channels_config,
                client_channels_config,
                ..Default::default()
            });

            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let client_id = current_time.as_millis() as u64;
            let server_addr = SocketAddr::new(ip, port);
            let socket = UdpSocket::bind((ip, 0))?;
            let authentication = ClientAuthentication::Unsecure {
                client_id,
                protocol_id: PROTOCOL_ID,
                server_addr,
                user_data: None,
            };
            let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

            commands.insert_resource(client);
            commands.insert_resource(transport);

            commands.spawn(TextBundle::from_section(
                format!("Client: {client_id:?}"),
                TextStyle {
                    font_size: 30.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        }
    }

    Ok(())
}


fn server_connection_events_system(
    mut commands: Commands,
    mut server_events: EventReader<ServerEvent>,
) {
    for event in &mut server_events
    {
        match event
        {
            ServerEvent::ClientConnected { client_id} => 
            {
                info!("Client '{client_id}' connected");

                commands.spawn((Player(*client_id), Position(Vec2::ZERO)));
            }
            ServerEvent::ClientDisconnected { client_id, reason } =>
            {
                info!("Client '{client_id}' disconnected: {reason}");
            }
        }
    }
}


