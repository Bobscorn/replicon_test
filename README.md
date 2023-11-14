# replicon_test
Project designed to test bevy_replicon's pre-spawning entities

## Found bug:
Sometimes an entitiy pre-spawned on a client will not match up with the one pre-spawned by the server, resulting in a second entity being spawned, and the server not knowing the first entity exists (and as a result it is not destroyed on the client when it is on the server).

### Expected:
When the client pre-spawns an entity and tells the server about it, the server should spawn its own copy, but replicate the data of this entity onto the client's pre-spawned entity, and not create a new copy on the client.

### Understood Cause:
My current understanding is that it is a discrepancy between the client receiving the server mappings _**after**_ it receives the new entity. This results in bevy_replicon spawning a new entity on the client (because it doesn't know that one already exists) and inserting the entity mapping linking the new entity, and not linking the existing entity.


Current guesses are:
- Large amount of traffic means sometimes the client mappings get sent after the spawned entity
- Packets are very small and result in a packet containing only the entity data, and not the client/server mappings

## Workaround
Current workaround is to attach a component onto the client's prespawned entity and monitor the entity for an entry in Server entity mapping. If it doesn't find a match after a set time, destroy the entity (and assume another copy was created instead).


## Results:

### main.rs:
Press space on the client to trigger a pre-spawn and input event to the server, then watch the count in the bottom right of both the client and server.

Produces the buggy behaviour very reliably, very high percentage of 'space' inputs pre-spawns an entity on the client, and then receives a replicated copy from the server

### test_2.rs:
Press space on the client to trigger a pre-spawn and input event to the server, then watch the count in the bottom right of both the client and server.

Produces the buggy behaviour if space is pressed whilst spamming Enter. This supports the large amount of traffic theory.


