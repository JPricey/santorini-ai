# UCI

This submodule implements the UCI Santorini AI executable. In this doc we cover the communication protocol of this process.

# Serialization Formats
## Board State
Board states are represented in this format:

```<height_map>/<current_player_id>/<player_details: player 1>/<player_details: player 2>```
- `height_map`: 25 digits representing the height map of the board. Each digit must be a number from 0-4 inclusive. Domes are always represented as 4s, even for techincally incomplete towers (this is a known limitation). The digits are ordered row-wise, starting with `A5` as in: `A5, B5, C5, D5, E4, A4...`
- `current_player_id`: either `1` or `2` representing whose turn it is
- `player_details`: A string in this format: `<god_name>[#][<optional god state>]:<worker_position>,...`. Broken down as:
    - First, a god name in lowercase
    - Then, optionally a `#` if the game is over and this player is the winner.
    - Then, optional square brackets with a god state string. The form of this state depends on the god, and is outlined below.
    - Then a `:`, marking the start of the worker positions section
    - Then a comma separated list of worker positions, represented as a file & rank coordinate (ex: A5).

Example: `4101202110011400102000100/2/mortal:A3,C3/artemis:E4,A1`

### God State

Some gods have powers that utilize state other than their own worker placements. An example of a game string using these gods is: `0000011000001000000000100/1/aeolus[w]:B3,C2/clio[1|B4,C3]:C3,D3`.  
The format of this state is described per relevant god:

#### Athena & Nike
Use `^` If Athena climbed on her last turn (or if Nike moved down on their last turn), and is now blocking upponents upwards movements. Defaults to non-blocking behaviour. Example: `athena[^]:A1`

#### Morpheus
Use the number of builds that Morpheus has stored. Defaults to 0. Stored builds are only updated at the _end_ of Morpheus' turn. This means that during his turn, Morpheus always has 1 more build available than indicated. Example: `morpheus[1]:A1` (if this were Morphreus' turn, he would have 2 builds available)

#### Clio
Clio represents both number of remaining coin placements, and current coin placements using this format: `<remaining coin placements>|<comma separated coin coordinates>`. Defaults to 3 remaining placements.
Example: `clio[1|B2,C3]:A1`

#### Europa
The coordinate representing the square that Talus is placed, or empty string. Defaults to no placement. Example: `europa[A2]:A1`

#### Aeolus
One of `n`, `ne`, `e`, `se`, `s`, `sw`, `w`, `nw`, or empty string. Represents the direction of movement that is currently _blocked_ (which is opposite the wind direction). Defaults to no wind. Example: `aeolus[e]:A1`

#### Selene & Hippolyta
The square of the female worker, or empty if there is none. Must match the location of a worker in the worker list. Example: `selene[A1]:A1,A2`


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

### Why do input vs output use a different serialization format?
- Mostly lazyness. I wanted output data to be highly structured for ease in parsing in another app, while being human readable enough, while the input format should be simple enough to write by hand without much boilerplate. Maybe later JSON will be accepted as inputs too.

# References
Chess UCI specification:
https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf
