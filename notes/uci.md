# UCI

Goals:
- Run engine with a suitable interface for these usecases:
    - A game UI
        - The engine must handle computing next states, and serialize an action chain to reach those states
    - A game analysis UI
    - As an agent for CPU vs CPU games.
        - Later, we'll introduce a test harness that spins up 2 different engines and competes them

# Serialization Formats
## Board State
Board states are represented in this format:

```<height_map>/<current_player_id>/<player_details: player 1>/<player_details: player 2>```
- `height_map`: 25 digits representing the height map of the board. Each digit must be a number from 0-4 inclusive. Domes are always represented as 4s (TODO: is that valid? as in, is a 3 vs non-3 height dome ever different?).
- `current_player_id`: either `1` or `2` representing whose turn it is
- `player_details`: A string in this format: `<god_name>[#]:<worker_position>,...`
    - First, a god name in lowercase
    - Then, optionally a `#` if the game is over and this player is the winner.
    - Then a `:`, marking the start of the worker positions section
    - Then a comma separated list of worker positions, represented as a 0-24 positional index.

Example:
`4112202311011420102000100/2/mortal:3,14/artemis:1,12`

### Implemented gods:
- `mortal`
- `artemis`
- `hephaestus`
- `pan`

### Position Index <> Coordinate mapping:
Often positions are refered to by a specific index (for example, worker positions). Positions map to board coordinates like this:
```
5 | 0  1  2  3  4
4 | 5  6  7  8  9
3 | 10 11 12 13 14
2 | 15 16 17 18 19
1 | 20 21 22 23 24
  +---------------
    A  B  C  D  E
```

### Interactive Player Actions
Player actions are the actual steps a player must take in order to transition from one state to the next.
Here are the set of possible interactive player actions:
```
{
    type: 'select_worker',
    selection: board_position_index,
}

{
    type: 'move_worker',
    selection: board_position_index,
}

{
    type: 'build',
    selection: board_position_index
}
```

### TODOs:
- Currently we represent domes as buildings with height 4. If a building with height < 3 gets domed (for example, by Atlas), we'll have to raise it to height 3 + dome it. Is that fine? This technically loses some information , but does this ever actualy impact gameplay?
- The board state does not include selected gods
    - Will probably change the player_details section to include a god name. Something like /mortal:1,3/apollo:5,6
- The board state does not currently have a representation for who won / lost. This isn't relevant yet with no gods - once you are at level 3 you have won - but does matter once gods are added that let you reach level 3 without winning.

# Commands
## Inputs
Commands are input in the format:
`command_name [arg1] [arg2]...\n`

The UCI must always be ready to accept commands, even while some other computation is in progress.

`set_position <board_state_fen>`: The engine will stop all other computation and start computing moves for this new position.
`next_moves <board_state_fen>`: The engine will output all board states reachable from moves board_state_fen, plus the incremental actions required to reach those board states.
`ping`: Returns `pong`
`stop`: Stops the current calculation, if in progress
`quit`: Closes the engine

## Outputs
Outputs are in JSON format.

### best_move
```
{
    "type": "best_move",
    "start_state": <board_state_fen>, // The position that this computation started from
    "next_state": <board_state_fen>, // The resulting position after this players action
    "meta": {
        "calculated_depth": <int>, // The depth that this move was calculated at
        "elapsed_seconds": <float>, // The time in seconds since the start of computation that it took to compute this move
        "actions": [...<player_action>], // list of interactive player actions to reach this state
    }
}
```

### next_moves
```
{
    "type": 'next_moves',
    "start_state": <board_state_fen>,
    "next_states": [
        {
            "next_state": <board_state_fen>,
            actions: [...<player_action>]
        },
        ...
    ],
}
```

### started
Started is emitted on startup, as soon as the engine is ready to receive commands.
```
{
    "type": 'started',
}
```

## Misc

### Command TODOs
- "status_check" command to check on the engine status
- Additional arguments to set_position
    - Time limit
    - Depth limit
    - If incremental best moves should be output

- More player actions, as gods are added
    - The harder part of adding new gods at this point is serializing their actions in a way that can be clearly represented in a UI.
    - Hermes probably requires a fully dedicated action for their turn
    - build needs a height property for Hephastus (or does it just build twice in a row??)
        - For atlas too??
    - build needs to support multiple destinations

Some gods will have a problem where there's multiple paths to end up in the same state.
Move generation is a single function with 2 variants to output intermediate actions or not.
Solutions are:
    - Separate fast vs slow move generation entirely (error prone, annoying to maintain)
    - Adhoc jank solutions (ex: for promethius, always move first and then possibly allow building under yourself)
    - Aggregator is responsible for dedupe, which we can run only in fast mode.
    - Move generation only finds unique solutions, and then we can add other solutions in post processing for UI only


### Why do input vs output use a different serialization format?
- Mostly lazyness. Thinking here is that as much data should be structured as possible, but forcing inputs to be JSON would make it a bit more annoying to test on a CLI. JSON outputs are human readable enough are will be much easier for a program to interpret. Maybe later we can add a JSON scheme for inputs.

# References
Chess UCI specification:
https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf
