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
<height_map>/<current_player_id>/<player_details: player 1>/<player_details: player 2>
height_map: 25 digits representing the height map of the board. Each digit must be a number from 0-4 inclusive. Domes are always represented as 4s (TODO: is that valid? as in, is a 3 vs non-3 height dome ever different?).
current_player_id: either `1` or `2`
player_details: Comma separated list of worker positions, represented as position index.

Example:
4112202311011420102000100/2/3,14/1,12

### Position Index <> Coordinate mapping:
```
5 | 0  1  2  3  4
4 | 5  6  7  8  9
3 | 10 11 12 13 14
2 | 15 16 17 18 19
1 | 20 21 22 23 24
  +---------------
    A  B  C  D  E
```

### TODOs:
- Currently we represent domes as buildings at height 4. If a <3 height building gets domed (for example, Atlas), we'll have to raise it to height 3 + dome it. Is that fine? Technically this is losing some information in how the game is rendered, but does this ever actuall impact gameplay?
- The board state does not currently have a representation for gods
    - Will probably change the player_details section to include a god name. Something like /mortal:1,3/apollo:5,6
- The board state does not currently have a representation for who won / lost. This isn't relevant yet with no gods, but does matter once gods are added that let you reach level 3 without winning.

# Commands
## Inputs
Commands are input in the format:
`command_name [arg1] [arg2]...\n`

The UCI must always be ready to accept commands, even while some other computation is in progress.

`set_position <board_state_fen>`: The engine will stop all other computation and start computing moves for this new position.
`next_moves <board_state_fen>`: The engine will output all board states reachable from moves board_state_fen, plus the incremental actions required to reach those board states.


## Outputs
Outputs are in JSON format.

`best_move`:
```
{
    "type": "best_move",
    "start_state": <board_state_fen>, // The position that this computation started from
    "next_state": <board_state_fen>, // The resulting position after this players action
    "calculated_depth": <int>, // The depth that this move was calculated at
    "elapsed_seconds": <float>, // The time in seconds since the start of computation that it took to compute this move
    "actions": [...<player_action>], // list of interactive player actions to reach this state
}
```

```
{
    "type": 'next_moves',
    "next_states": [
        {
            "next_state": <board_state_fen>,
            actions: [...<player_action>]
        },
        ...
    ],
}
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
    selection: [...board_position_index],
}
```

### Command TODOs
- "status_check" command to check on the engine status
- Additional arguments to set_position
    - Time limit
    - Depth limit
    - If incremental best moves should be output

- More player actions, as gods are added
    - Hermes probably requires a fully dedicated action for their turn
    - build needs a height property for Hephastus (or does it just build twice in a row??)
        - For atlas too??


### Why do input vs output use a different serialization format?
- Mostly lazyness. Thinking here is that as much data should be structured as possible, but forcing inputs to be JSON would make it a bit more annoying to test on a CLI. JSON outputs are human readable enough are will be much easier for a program to interpret. Maybe later we can add a JSON scheme for inputs.

Input commands:
- "status_check"
    - UCI is expected to respond with a "status_resp" command
- "set_position"
    - "board": FEN string
    - // Add a mode/limits later. For now always use "incremental" mode, which outputs best moves as they are discovered
- "stop"
    - change state from thinking to pending

// TODO: commands for a GUI

Output commands:
- "status_resp"
    - "initializing", "pending", "thinking"
- "best_move"
    - "current_state"
    - "best_child"
    - "meta"
        - "depth"
        - "elapsed"

// TODO: commands for a GUI

Punting on :
- Setting engine options
- Ponder
- UI stuff


# References
Chess UCI specification:
https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf
