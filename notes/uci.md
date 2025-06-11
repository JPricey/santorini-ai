# UCI

Goals:
- Run engine with a suitable interface for these usecases:
    - A game UI, potentially in a different language
    - An engine UI for position analysis
    - As an agent for CPU vs CPU games.
        - Later, we'll introduce a test harness that spins up 2 different engines and competes them

Interface:
Based on UCI for chess;
https://gist.github.com/DOBRO/2592c6dad754ba67e6dcaec8c90165bf

- UCI must always be available to accept commands. Main thread will be dedicated to IO
- Commands are sent as JSON, with the structure:
```
{
    "command": "<command_name>",
    ...other args
}
```

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
