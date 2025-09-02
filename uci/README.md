# UCI

This submodule implements the UCI Santorini AI executable. In this doc we cover the communication protocol of this process.

# Serialization Formats
## Board State
Board states are represented in this format:

```<height_map>/<current_player_id>/<player_details: player 1>/<player_details: player 2>```
- `height_map`: 25 digits representing the height map of the board. Each digit must be a number from 0-4 inclusive. Domes are always represented as 4s, even for techincally incomplete towers (this is a known limitation)
- `current_player_id`: either `1` or `2` representing whose turn it is
- `player_details`: A string in this format: `<god_name>[#]:<worker_position>,...`
    - First, a god name in lowercase
    - Then, optionally a `#` if the game is over and this player is the winner.
    - Then, optionally a `-` if the opponent is Athena, who just climbed
    - Then a `:`, marking the start of the worker positions section
    - Then a comma separated list of worker positions, represented as a file & rank coordinate (ex: A5).

Example:
`4101202110011400102000100/2/mortal:A3,C3/artemis:E4,A1`

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
    "original_str": <board_state_fen>, // The exact input board string given to trigger this command
    "start_state": <board_state_fen>, // original_str, possibly rewritten to a more canonical format
    "next_state": <board_state_fen>, // The resulting position after the chosen action
    "trigger": <trigger_string>, // enum representing what prompted this output
    "meta": {
        "score": <int>, // The predicted score from the POV of the current player. Estimated win probably is `sigmoid(score/400)`
        "calculated_depth": <int>, // The depth that this move was calculated at
        "nodes_visited": <int>, // The number of nodes visited in this calculation
        "elapsed_seconds": <float>, // The time in seconds since the start of computation that it took to compute this move
        "actions": [...<player_action>], // list of interactive player actions to reach this state
        "action_str": <string>, // A string representing the actions taken this turn
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

* next_states may contain duplicate states, with multiple paths to get there

### started
Started is emitted on startup, as soon as the engine is ready to receive commands.
```
{
    "type": 'started',
}
```

#### Subtypes
##### trigger_string types
Once `set_position` is called, the engine will continuously output new best move predictions until told to stop. These triggers may be one of:

```
saved: may be output as the first message in a search, if this state has a saved best move from a previous search
stop_flag: output when the engine is requested to stop
improvement: output when the engine finds a better move, or a more accurate prediction of the last output move
end_of_line: output when the engine finds a mate that it can't find a refutation for
```

##### Interactive Player Actions
Player actions are the actual steps a player must take in order to transition from one state to the next.
Here are the set of possible interactive player actions:
```
{
    type: 'select_worker',
    value: <coordinate>,
}

{
    type: 'place_worker',
    value: <coordinate>,
}

{
    type: 'move_worker',
    value: <coordinate>,
}

{
    type: 'move_worker_with_swap',
    value: <coordinate>,
}

{
    type: 'move_worker_with_push',
    value: [<coordinate for own worker move>, <coordinate for pushed worker move>],
}

{
    type: 'build',
    value: board_position_index
}

{
    type: 'dome',
    value: board_position_index
}

{
    type: 'no_moves',
}
```

## Misc

### Known Limitations
- Domes are represented as spaced with height 4. This means that incomplete towers cannot be represented
- No characters with tokens have been implemented yet. The board state format will have to be extended for this purpose. When this happens athenas height signal will be changed as well.

### Why do input vs output use a different serialization format?
- Mostly lazyness. I wanted output data to be highly structured for ease in parsing in another app, while being human readable enough, while the input format should be simple enough to write by hand without much boilerplate. Maybe later JSON will be accepted as inputs too.

# References
Chess UCI specification:
https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf
