# Santorini AI

Game engine for the board game Santorini using negamax search with NNUE evaluation.

### Workspace crates (`Cargo.toml`)
- **`santorini_core`** - Core game logic, gods, search, NNUE eval. The heart of the project.
- **`uci`** - UCI-like protocol interface for external UIs
- **`ui`** - Native analysis GUI built with egui
- **`wasm_app`** - WASM bindings for the web app
- **`battler`** - Runs automated games between engine configurations
- **`datagen`** - Generates training data for NNUE from self-play
- **`bullet_prep`** - Prepares NNUE training data in bullet format

### Other directories
- **`web_app/`** - TypeScript/Vite web frontend (deployed to GitHub Pages)
- **`python/`** - Data analysis scripts (`pull_data.py`, `read_matchups.py`, `ui.py`)
- **`game_data/`** - Generated game data files from datagen runs
- **`models/`** - NNUE model files (`.nnue` binary format)
- **`data/`** - Matchup configuration YAML files

### Key binaries in `santorini_core/src/bin/`
- `fuzzer.rs` - Fuzz testing for game logic consistency
- `visit_tester.rs` - Tests search visit counts across positions
- `tree_perf.rs` - Performance benchmarking for search tree traversal
- `post_process_model.rs` - Post-processes NNUE model files

### Battler binaries (`battler/src/bin/`)
- `run_matchups.rs` - Runs batch matchups between god pairs
- `compare_engines.rs` - Compares two engine configurations
- `faceoff.rs` - Runs a face-off between two specific configurations
- `single.rs` - Runs a single game between configurations
- `seed.rs` - Generates seed positions

## Core Game Model

### Board Representation (`board.rs`)
The 5x5 Santorini board is represented using bitboards (32-bit integers via `BitBoard`).

- **`BoardState`** - The core game state:
  - `current_player: Player` - Whose turn it is (`Player::One` or `Player::Two`)
  - `height_map: [BitBoard; 4]` - Four bitboard layers encoding building heights. `height_map[L-1]` has bit set for squares at height >= L (levels 1-4, where 4 = dome)
  - `workers: [BitBoard; 2]` - Worker positions per player (bitboard with bits set for worker squares)
  - `god_data: [u32; 2]` - God-specific state per player (e.g., Athena's "opponent can't climb" flag, Morpheus block count, Aeolus wind direction)
  - `hash: HashType` - Zobrist hash for transposition table
  - `height_lookup: [u8; 25]` - Cached height per square

- **`FullGameState`** - Combines `BoardState` with a `GodPair` (two `&'static GodPower` references). This is the primary state type used throughout the engine. Serializes as a FEN string.

- **`Square`** - Enum of 25 squares (A5..E1), stored as `u8`, indexed row-major from top-left.

- **`BitBoard`** - 32-bit bitboard. Bits 0-24 map to the 25 squares. Bits 25-29 reserved. Bits 30-31 encode winner state (in `height_map[0]`).
- Many square -> neighbor / push mappings (NEIGHBOR_MAP, INCLUSIVE_NEIGHBOR_MAP, WRAPPING_NEIGHBOR_MAP...) are precomputed.

### Move Representation
Moves are encoded as `GenericMove(u32)` - a 32-bit integer with bit-packed fields. Each god defines its own move struct (e.g., `MortalMove`, `ApolloMove`) that transmutes to/from `GenericMove`.

Typical fields packed into the u32:
- `move_from_position` (5 bits) - worker starting square
- `move_to_position` (5 bits) - worker destination square
- `build_position` (5 bits) - where to build
- Additional god-specific fields (e.g., swap square for Apollo)
- Bit 31 (`MOVE_IS_WINNING_MASK`) - flags the move as a win
- Bit 30 (`MOVE_IS_CHECK_MASK`) - flags the move as creating a check (threatening to win)

`ScoredMove` pairs a `GenericMove` with a `MoveScore` for ordering during search.

### FEN Format (`fen.rs`)
Board states serialize as FEN strings: `heights/current_player/god1:workers god2:workers`
Example: `10000 00000 00000 00000 00000/1/mortal:A1,A2 pan:E4,E5`

## Implementing Gods

### Architecture Overview
Gods are implemented as **static function pointers** assembled into a `GodPower` struct, not as trait objects. The `GodPower` struct holds ~20 function pointers covering move generation, move application, placement, and serialization.

### Pattern for Adding a New God

1. **Create a new file** `santorini_core/src/gods/your_god.rs`

2. **Define a move struct** that wraps `MoveData` (u32):
```rust
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct YourGodMove(pub MoveData);
```
Pack move fields (from_square, to_square, build_square, etc.) into the u32 using bit shifts. Use `POSITION_WIDTH` (5 bits) per square.

3. **Implement `GodMove` trait** for your move struct:
   - `move_to_actions()` - Converts the compact move into a `Vec<FullAction>` (list of `PartialAction` steps for the UI)
   - `make_move()` - Applies the move to a `BoardState` mutably. Call `board.worker_xor()` to move workers, `board.build_up()` to build, `board.set_winner()` for wins
   - `get_blocker_board()` - Returns a `BitBoard` of squares this move interacts with during a winning move to help with blocking checks during search (behaviour when not winning is undefined).
   - `get_history_idx()` - Returns a unique index for the move (used for history heuristic in search)

4. **Implement `Into<GenericMove>` and `From<GenericMove>`** using `unsafe { std::mem::transmute(self) }`

5. **Write a move generator function** with this exact signature:
```rust
pub(super) fn your_god_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove>
```
The `F` const generic controls behavior (mate-only, include scores, interact with key squares). `MUST_CLIMB` handles Persephone's power. Use helpers from `move_helpers.rs`:
   - `get_generator_prelude_state()` - Sets up common data (worker positions, height masks, etc.)
   - `get_worker_start_move_state()` - Per-worker setup
   - `get_worker_next_move_state()` / `get_basic_moves()` - Get legal move destinations
   - `get_worker_end_move_state()` - Per-destination setup
   - `get_worker_next_build_state()` - Get legal build positions
   - `push_winning_moves()` - Emit winning moves, returns true if should stop early
   - `build_scored_move()` - Create a scored move with check/improver flags
   - Start the function with `persephone_check_result!()` macro to handle Persephone interaction

6. **Write a `build_your_god()` function**:
```rust
pub const fn build_your_god() -> GodPower {
    god_power(
        GodName::YourGod,
        build_god_power_movers!(your_god_move_gen),
        build_god_power_actions::<YourGodMove>(),
        <unique_hash_1>,  // random u64
        <unique_hash_2>,  // random u64
    )
}
```
Chain `.with_*()` methods for special behaviors (custom win mask, build mask, placement type, god data parsing, etc.).

7. **Register the god**:
   - Add variant to `GodName` enum in `gods.rs` with next sequential integer value
   - Add `pub(crate) mod your_god;` to the module list in `gods.rs`
   - Add `your_god::build_your_god()` to `ALL_GODS_BY_ID` array (must be at the index matching the enum value)
   - If WIP, add to `WIP_GODS` array
   - If god uses `god_data`, add to `god_name_to_nnue_size()` and update `TOTAL_GOD_DATA_FEATURE_COUNT_FOR_NNUE`

8. **Add tests** - Use the consistency checker (`consistency_checker.rs`) which validates move generation against brute-force enumeration. Tests typically use FEN strings to set up positions.

### Key Concepts for Move Generation

- **Prelude state**: Common computed data shared across all worker iterations (height masks, neighbor maps, worker positions, opponent info)
- **Check detection**: A move is a "check" if after the move, the current player threatens to win next turn. This is computed by looking at reachable level-3 squares
- **Improving moves**: Moves where the worker moves to a higher level (used for move ordering)
- **Key squares**: Squares the opponent's winning moves interact with. Used by `INTERACT_WITH_KEY_SQUARES` mode to generate only relevant blocking moves
- **MoveGenFlags**: Const generic bitflags controlling which moves to generate:
  - `MATE_ONLY` - Only generate winning moves
  - `STOP_ON_MATE` - Return immediately when a win is found
  - `INCLUDE_SCORE` - Compute move scores for ordering
  - `INTERACT_WITH_KEY_SQUARES` - Only generate moves that interact with given key squares
- **Persephone macro**: `persephone_check_result!()` handles Persephone's power (forces opponent to move up if possible) by recursively calling the move gen with `MUST_CLIMB=true`

### God-Specific Features
- **Custom placement**: Override via `.with_placement_type()` (ThreeWorkers, PerimeterOnly, FemaleWorker, etc.)
- **God data**: Per-player u32 stored in `BoardState.god_data[]`. Used for stateful powers (Athena's climb restriction, Aeolus wind direction, Morpheus block count). Requires implementing parse/stringify/flip functions
- **Custom win conditions**: Override `win_mask` to change which squares count as winning
- **Build restrictions**: Override `_build_mask_fn` to restrict where the god can build
- **Opponent interaction**: `_can_opponent_climb_fn` (Athena), `_moveable_worker_filter_fn` (Hypnus), `is_aphrodite`, `is_persephone` flags

## Search System (`search.rs`)

### Algorithm
Negamax with alpha-beta pruning, iterative deepening, and aspiration windows.

### Key Components
- **`SearchContext`** - Holds transposition table, callback for new best moves, and terminator
- **`negamax_search()`** - Entry point. Does iterative deepening from depth 1, each iteration with aspiration windows. Falls back to full window on fail-high/fail-low
- **`_negamax()`** - Recursive search function with:
  - Transposition table lookup/store
  - Null move pruning
  - Reverse futility pruning (static eval margin)
  - Late move reductions (LMR)
  - Killer move heuristic (2 killers per ply)
  - History heuristic for move ordering
  - Quiescence-like extension for winning/blocking moves at leaf nodes
- **Move ordering** (`move_picker.rs`): TT move first, then killers, then by history score. `MovePicker` yields moves lazily via `pick_next()`

### NNUE Evaluation (`nnue.rs`)
- Efficiently updatable neural network for position evaluation
- Features: worker positions (relative to player perspective), building heights, god-specific data
- Uses SIMD (portable_simd) for inference
- Model loaded from embedded binary data (`models/`)
- `NNUEState` tracks accumulated features and is incrementally updated as moves are made/unmade

### Transposition Table (`transposition_table.rs`)
- Fixed-size hash table mapping Zobrist hashes to search results
- Stores: best move, score, depth, node type (exact/alpha/beta)
- Uses replacement scheme based on depth

### Search Terminators (`search_terminators.rs`)
- `StopFlagSearchTerminator` - Checks an `AtomicBool` flag (used by engine thread)
- `StaticMaxDepthSearchTerminator<N>` - Stops at depth N
- `StaticNodesVisitedSearchTerminator<N>` - Stops after N nodes
- Combinators: `AndSearchTerminator`, `OrSearchTerminator`

## Engine Thread (`engine.rs`)
- `EngineThreadWrapper` manages a background search thread
- Communicates via channels (`EngineThreadMessage::Compute/End`)
- `start_search()` begins, `stop()` halts and returns best move
- `search_for_duration()` runs for a specified time
- Transposition table persists across searches within the same thread

## Matchups (`matchup.rs`)
- `Matchup` represents a god-vs-god pairing
- Matchups are always stored in sorted order (lexicographic by god name)
- `BANNED_MATCHUPS` list defines disallowed god pairings
- Matchup data (win rates, etc.) stored in YAML files in `data/`

## Fuzzer (`santorini_core/src/bin/fuzzer.rs`)
Plays random games and runs the consistency checker on every position. Flags:
- `-g` / `-G` — god name(s) for player 1 / player 2 (omit for all gods)
- `-s` — stop on first failure
- `-t` — timeout in seconds

```bash
cargo run -p santorini_core --bin fuzzer -r -- -g morpheus -s -t 30
```

## Consistency Checker (`consistency_checker.rs`)
Validates god implementations by checking move generation against brute-force move enumeration. Verifies that:
- All legal moves are generated
- No illegal moves are generated
- Move application produces correct board states
- Win detection is correct
- Move scoring is consistent
