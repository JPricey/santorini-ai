# Charybdis — implementation plan

## Implementation status (as built)

Landed on `dev`, engine + UI building, 174 tests green, and the fuzzer clean against every audited
opponent (45 gods). **The whole displacement family is done** - every god that relocates, removes
or places a worker now faces her. What is left is the multi-step movers and four concrete rulings.

**Prometheus was briefly re-banned** when this pass found him generating illegal teleports, then
fixed along with Achilles - see "The pre-build family". He is the only audited god that was ever
wrong, and the bug was invisible to the fuzzer, which is the second time that blind spot has bitten.

**Built as planned:** outcome encoding (D1), the portal swap in the shared movement funnel (§5.3),
the height rule from §1a (entry-only restrictions, win-check-only fiction), `mate_start_mask` /
`can_mate` (§5.4), the central build-clears-tokens hook (§5.6), portal-aware blocker boards (§5.7),
her own generator with token placement (§5.8), Mortal NNUE proxy (§5.9), and a
`PartialAction::PlaceWhirlpool` wired through the egui UI and the web app (§5.10). Whirlpools also
render as tokens on both UIs, alongside the Talus and female-worker markers.

**Changed during implementation:**

- **`winnable_squares` (new, not in the plan).** §5.4's widened guard turned out to be unsound on
  its own: a worker *standing* on level 3 (reachable via Hera or displacement) would have had its
  flat level-3 move tagged as a win. The win extraction now intersects a per-worker arrival mask -
  everything for a level 2 worker, the portal exit only for anybody else. 48 sites.
- **`get_blocker_board` did not need a `player` argument.** Adding the portal squares is
  god-independent, so it happens once in `GodPower::get_blocker_board` instead of in 42 impls.
- **Check tagging is approximate, and the consistency checker knows it.** A build that lands on a
  whirlpool, or a placement that creates one, rewrites the threat map *after* the reach board was
  computed. Making the flags exact means recomputing threats per (build, placement) pair. Both
  sides of a Charybdis matchup are exempted from the check-flag validation, as Triton and
  Stymphalians already are.
- **Blocker generation deliberately over-generates**, for the same reason: whether a whirlpool
  block actually blocks depends on the opponent's replies. Over-generating costs nodes;
  under-generating loses games. Exempted in the checker with that rationale.
- **A general Morpheus bug fell out**: he could decline to build, but blocking generation dropped
  his zero-build moves whenever no build could touch a key square. Fixed for all matchups.

**Audited opponent list: 45 gods**, each fuzzed against Charybdis individually *and* covered by
`test_every_audited_opponent_routes_moves_through_the_portal`, which is the check that actually
matters - see below. Everything else is
banned as `BannedReason::Engine`. Beyond the phase 1 set this now includes Graeae, Maenads, Bia,
Nike, Eros, Hydra, Urania and both Chronus variants - that batch cost two bug fixes
(below) and no design changes, which is the evidence that the funnel approach in §5.3 holds - plus
six displacement gods, **Apollo, Minotaur, ApolloV2, Scylla, Charon and CharonV2** (see
"Displacement family" below), and both pre-build gods, **Prometheus and Achilles**.

**Fuzzing cannot clear a god for this power.** A god that never routes destinations through the
portal emits a move list that is entirely self-consistent - it is simply missing the teleport - so
every invariant the checker knows about still holds. Ten gods were "cleared" on that basis and the
clearance meant nothing. The real criterion is whether a god's movement goes through
`get_limited_moves_given_move_mask`; the test above asserts the observable consequence of that, by
reading outcomes off the board rather than out of the move encoding (every god packs its move
struct differently, so decoding `move_to_position` only works for gods sharing Mortal's layout).

Auditing that criterion caught **iris and urania** ignoring whirlpools entirely. Urania is fixed.
Iris is fixed and then fails differently - see below.

Four are banned for concrete reasons rather than "not audited yet":

- **Europa** - the Talus and a whirlpool can occupy the same square, and neither card says what
  that square then is.
- **Persephone** - a whirlpool mate whose entry is a step *down* is a non-climbing win, so handing
  Charybdis a climb anywhere suppresses it. That is a real block that has nothing to do with the
  squares the win touches, so blocker generation cannot be asked for it.
- **Harpies** - her slide is forced movement, which by the card does not trigger a whirlpool, but a
  slide that ends on one, or starts from one, needs an ordering ruling nobody has written down.
- **Iris** - she reaches a whirlpool by jumping over a worker, so that worker is a stepping stone
  and walking it away defuses the win. Key square narrowing matches on where a move *lands*, so it
  cannot express that block, and the search would miss the defence.

Two more general bugs fell out of that batch:

- **Eros' placement generator never checked its first square against the opponent's workers**, so
  it could stack two workers on one space. Pre-existing, affects every Eros matchup, fixed.
- **A flush-back win crashed the consistency checker.** A worker that steps into a whirlpool and is
  flushed back onto its own square never appears to have moved, and the win validator asserts it
  can identify the moved worker.

### Displacement family (Apollo, Minotaur, ApolloV2, Scylla, Charon, CharonV2 done; the rest next)

A displacer stepping onto a whirlpool is a **three-square outcome**: the mover teleports to the
exit, the displaced worker goes where the god's rule sends it, and the entry empties. Two pieces of
shared machinery were wrong for this, and both are now fixed:

- **The funnel's portal swap** (`get_limited_moves_given_move_mask`, §5.3) remaps entry->exit for a
  *lone* mover. For a displacer it double-applies *and* discards the entry the god still needs to
  displace. A new `APPLY_PORTAL` const generic (default true) lets a displacer take the **raw
  entries** and remap them itself via the shared `displacer_portal_exit(portal, exit_blockers,
  entry)`, keying displacement off the entry and build/reach/height/win off the exit.
- **`get_active_portal` is too strict** - it disarms when *either* whirlpool is occupied. A
  displacer only needs the *exit* free, since it clears the entry as it steps on. `displacer_portal_exit`
  encodes that (only the exit is checked), and the win validator no longer requires the entry empty,
  only the exit. (The pre-committed `get_active_portal_after_displacement` primitive is not the
  right shape for a *stepping* displacer - a per-entry exit is what those need - but it turned out
  to be exactly right for the **pull** family, see Charon below.)

**The displacement family splits in two, by when the displacement resolves.** A god that displaces
by *stepping onto* the victim (Apollo, Minotaur) needs the per-entry `displacer_portal_exit`. A god
that relocates somebody *before or after* its own move (Charon pulls first, Scylla drags after) is
still a lone mover at the moment it teleports - it just needs the portal judged against the right
board. That is `get_active_portal_after_displacement(vacated, newly_filled)`, plus
`winnable_squares_for_arrival`, which recomputes the win mask from the portal a move will actually
arrive into rather than the one the prelude saw.

Per-god notes:

- **Apollo** needed no move-struct change: it already stores `swap_from` separately from `move_to`,
  so the three-square outcome encodes as `move_to = exit`, `swap_from = entry`.
- **Minotaur** assumed the pushed worker sat on `move_to`, which the teleport breaks. But the push
  is always straight, so the origin is `BETWEEN_MAPPING[from][push_to]` - read off the geometry,
  unchanged for an ordinary push and correct for a teleported one. No move-struct change; the
  `make_move` change touches every Minotaur game, so it was also fuzzed 150s vs all opponents.
- **ApolloV2** was the mechanical follow-on Apollo promised: the same `ApolloMove` struct and
  `make_move` (so no off-portal re-fuzz was needed, only a 300s Charybdis run plus a 60s off-portal
  sanity run), the same `no_portal` + `displacer_portal_exit` rewrite in the generator. Its one
  wrinkle is the V2 "can't swap up" blocker (`oppo_workers & height_map[start_height]`), which is
  kept verbatim - it filters entries and never touches whirlpool squares, which can't be occupied.
  Its reach board already went through the portal-aware `get_standard_reach_board_from_parts`, so
  that needed nothing. Three `ApolloMove` accessors were widened to `pub(crate)` so the test can
  live in `apollo_v2.rs`.
- **Scylla** was nearly free, because her drag target is the square she *vacated* - which the
  teleport never touches - so `make_move` was already correct and she stays a lone mover for portal
  purposes. Two gaps: the shared `_no_affinity` helper dropped `worker_start_mask`, which wrongly
  disarmed the portal whenever the mover was itself standing on a whirlpool (it only ever reached
  Scylla and Charon, both banned, so no audited god was affected); and a worker flushed *back to its
  own square* never vacates it, so the drag had to be suppressed there or it stacks two workers.
- **Charon** is the first god whose displacement changes the portal *before* he moves, so whether
  the teleport exists depends on which flip he picks: pulling a worker off a whirlpool arms the
  portal, pulling one onto a whirlpool shuts it. His base move set is therefore raw entries, with
  the swap and the winnable-square mask recomputed per flip. Three bugs fell out, two found by the
  fuzzer and one by re-reading:
  - His MATE_ONLY narrowing kept only flips that vacate a **level 3** square. A flip that frees a
    *whirlpool* also opens a mate - from a worker at any height - so it now keeps
    `exactly_level_3 | portal_squares`.
  - **A general win-validator bug**: the checker judged "was the exit free" on the *pre-move* board,
    so it rejected any portal win whose exit had been cleared earlier in the same move. It now asks
    which workers held the square both before and after, matched per player.
  - His "don't win by flipping what you could win without flipping" prune removed the winning
    *entry* from the base set. In entry space that is wrong: the same entry leads somewhere else
    under a flip that shuts the portal, so a legal quiet move was silently lost. Now tracked in
    outcome space instead.

- **CharonV2** was free, and worth stating why: he either moves normally *or* pushes **instead of**
  moving. The move branch is an ordinary lone move through the shared funnel, and on the flip branch
  he never leaves his square, so no rearrangement of the whirlpools can teleport him. Unbanned with
  tests asserting both halves and no generator change at all.

Each has targeted displacement-outcome and portal-win tests, and each fuzzed clean 300s against
Charybdis plus an off-portal run. The routing guard test caught a double-swap bug mid-development
that the fuzzer could not (a missing teleport still produces a self-consistent move list).

**Still banned:**

Thirteen matchups, all `BannedReason::Engine`, in two groups.

- **Nine multi-step movers: Artemis, Hermes, Triton, Pegasus, Castor, Proteus, Bellerophon,
  Stymphalians, Terpsichore.** The portal is a teleport *edge* that must apply at each step, which
  turns "reachable set" into a graph traversal. This is the whole remaining engine debt, and the
  hardest of it - several may stay banned a while. Two things learned from the displacement family
  transfer directly: generate from a *materialised* post-step board where that is affordable (the
  Odysseus trick), and check what `get_blocker_board` returns for every winning encoding, since a
  move that does not decompose into from/to silently yields a garbage blocker board (the Jason
  trap).
- **Four concrete rulings: Harpies, Europa, Persephone, Iris**, for the reasons given above. These
  are not "not audited yet" - each needs a decision, not an implementation.

**Other known follow-ups:** exact check tagging; the per-god reach-board sites that still recompute
`is_now_lvl_2` inline (Iris, Bia, Urania, Prometheus, Apollo, Achilles, Artemis - move ordering
only); and NNUE features for the whirlpool bitboard at the next retrain (she still proxies Mortal,
so she plays legally but evaluates blind to whirlpools - the biggest single lever on her strength).

### Removal, placement, mass forcing and the whole-board swap

- **Nemesis** was free, and the "hardest of the group" guess was wrong. His swap resolves *after*
  he has moved and built, so he is an ordinary lone mover at the moment he teleports, and
  `make_move` reads the swapped worker's square as `move_to_position` - which is the exit, so the
  teleport is already accounted for. The swap itself is forced movement, which by rule 6 never
  triggers a whirlpool, so a worker swapped *onto* one simply stands there.
- **Theseus** was free. His kill is read off where he *ends*, so it resolves after the move and he
  is an ordinary lone mover at the moment he teleports - the Scylla shape. Unbanned with a test and
  a clean 300s fuzz, no generator change.
- **Odysseus** was free, for a reason worth copying: his generator *materialises* the
  post-displacement board (`displaced_state`) and generates from that, so the entire prelude -
  `portal_squares`, `get_active_portal`, `winnable_squares` - is recomputed against the board he
  actually moves on. That is what Charon had to be taught by hand. His blocker board was already
  written in terms of real from/to squares plus the corners the move uses, so the Jason trap below
  did not apply. Unbanned with a test and a clean 300s fuzz, no generator change.
- **Jason** needed a move-struct change, the first of this whole effort. His hero *appears* on the
  board rather than moving, and the encoding stores where it ends up rather than where it was
  placed (placements reaching the same square are the same move). The generator carried the comment
  *"A placed worker starts on the ground, so it can never reach level 3 in one move"* - which the
  portal falsifies, since a whirlpool exit wins from any height. Three things followed:
  - **A win that was not a win.** Three moves landed the hero on a level 3 exit and none was
    flagged; mate generation found nothing, because the whole power path was gated behind
    `!is_mate_only`. Now there is a winning-placement encoding, `make_move` adds the worker and
    sets the winner, and the power path runs under MATE_ONLY when a portal is on the board.
  - **The win validator could not read it.** It asserts it can identify the worker that moved, and
    a hero win vacates nothing. It now recognises a placement landing on a free level 3 exit.
  - **Blocking it needed both halves.** `get_blocker_board` returned `move_mask()`, which for a
    hero move reads two unset fields - it was handing back square A5. Blocking a hero portal win
    means disarming the portal *or* standing on the square he would have been placed on, and the
    encoding deliberately does not record which placement was used, so the blocker board offers
    every ground level perimeter square along with the exit (the shared hook widens that to the
    whole portal). Both blocks came from the fuzzer, one after the other.

### The pre-build family (Prometheus and Achilles, both fixed)

Exactly two gods build **before** they move: Prometheus and Achilles. That ordering interacts with
whirlpools in a way nothing else does, because a build that lands on a whirlpool **returns that
token to Charybdis' supply** (§5.6) - so a pre-build can destroy the portal that the same turn's
move was about to use.

Prometheus had been audited and shipped, and was wrong. Both symptoms came from one category error:
**the pre-build path filtered the already-swapped destination set by height.**

1. **He used a portal he had just destroyed.**
   `0000000000000000000000000/2/charybdis[C3,E5]:A1,A2/prometheus:B3,E1` generated `^C3 B3>E5^D4` -
   pre-build on whirlpool C3, leaving a lone whirlpool and therefore no portal, then teleport
   B3 -> E5 anyway. E5 was never adjacent to B3.
2. **He dropped legal pre-build moves**, because "cannot move up" was judged on the *outcome* rather
   than the entry leg it belongs to (§1a).
   `0000200000000000000000000/2/charybdis[C3,E5]:A1,A2/prometheus:B3,E1` - whirlpools C3 (level 0)
   and E5 (level 2). Stepping flat into C3 and surfacing on E5 is not a climb, so it is legal with a
   pre-build; he emitted 3 plain moves onto E5 and **zero** pre-build ones. Now 18.

Neither was visible to the fuzzer: an extra teleport and a missing teleport both produce
self-consistent move lists, and the routing guard only checks a god's *plain* move, which Prometheus
routed correctly all along.

**The fix, for both gods: build the pre-build destination set in *entry* space and apply the swap
last**, with the portal treated as disarmed for exactly those pre-builds that land on a whirlpool.
Off-portal it is a strict no-op - `active_portal` is empty, entries and outcomes coincide, and the
second `get_basic_moves` walk is skipped entirely - which is what makes the change safe to make in
a hot path.

Achilles needed two things Prometheus did not:

- **He keeps his climb after the power build**, so a winning square can appear in his pre-build move
  set. That must never be emitted as a quiet move, or the win goes unrecorded - `make_move` would
  just build and move. His pre-build outcomes are therefore split into winning and quiet squares.
  (Every such win turns out to also be available *without* the power build, since from level 2 every
  buildable height is already a legal step, so this is a guard rather than a new source of moves.)
- **A pre-existing bug of his own, exposed by the portal.** His signature power mate - build a level
  2 square up to 3, then step onto it - is only a win from level 2. `MATE_ONLY` used to guarantee
  that for free, because only level 2 workers were ever acting workers. The portal widens
  `mate_start_mask` to *every* worker (§5.4), so a level 1 worker was being handed a two-level
  climb: `1111110100202103213111102/2/charybdis[A2,E2]:E4/achilles:B3,A1` produced `^B2 A1>B2#`.
  The height is now checked explicitly. **This one the fuzzer did catch**, because an impossible
  climb is not self-consistent - unlike a wrong teleport.

The routing guard test also had to be relaxed: it asserted that no worker ends on the whirlpool it
entered, which a pre-build god can legitimately do once it has handed that token back. It now only
reads moves that leave both whirlpools standing.

**The rest of the audited list was swept for the same pattern** - taking the post-swap outcome set
and re-filtering it by a rule that belongs to the entry leg. Prometheus and Achilles are the only
two instances, because they are the only two gods that build before moving. The other gods that
re-filter `worker_moves` after the fact (Iris, Bellerophon, Proteus, Terpsichore) are all banned for
other reasons already, so nothing else audited is exposed.

### The crash hunt (separate investigation, no Charybdis bug)

While looking into a reported "engine crash during play", built `search_fuzzer` (a bin that plays
whole games by *searching* each move - the ordinary fuzzer only plays random moves and never calls
`negamax_search`, so it cannot see search-path crashes). It reproduced a `build_up`-on-dome abort,
which turned out to be a **bug in the tool, not the engine**: on a lost position the engine returns
`action == NULL_MOVE` paired with a winner-set `child_state`, and re-applying that NULL action
builds garbage. Every real consumer (native UI, web app, battler) uses `child_state`, not the
action, so none crash. With the tool fixed to consume `child_state`, extensive tall-board search
hunts found nothing. No engine bug was found; if the user's crash recurs it needs the god/matchup
and any panic text. Latent footgun worth noting: the engine hands back a NULL action on lost
positions, which is a landmine for any future consumer that applies `.action` instead of
`.child_state`.

---

## 0. Status of prior art

There is an old branch `charybdis` (merge base `4aaf93b0 v0.1.23`, ~35 commits behind current `dev`).
It contains:

- `santorini_core/src/gods/charybdis.rs` (410 lines) — move struct, move gen, god-data parse/stringify/flips.
- `GodPower::_get_token_mask` + `with_get_token_mask_fn` + `is_token_user` flag (`gods.rs`).
- `GeneratorPreludeState.charybdis_tokens` — plumbed in but **never consumed** by any opponent god.
- `put_moves_through_portals(moves, portals)` in `move_helpers.rs` — the `count_ones() == 1 → xor` trick.
- `get_blocker_board(board, action)` → `get_blocker_board(board, action, player)` signature change.
- `build_up(square)` → `build_up(square, player, other_god)` so builds could clear tokens.

What that branch got right: the god-data representation, the portal swap trick, the blocker-board
extension, and the recognition that builds must clear tokens. What it never did: make the power
apply to **the opponent's** turn at all (which is most of the work), win/mate detection through the
portal, and blocking-move generation. Treat it as a source of snippets, not a base to rebase.

Salvage list (port, don't rebase):
`charybdis.rs` skeleton, `_get_token_mask`, `put_moves_through_portals`, the `get_blocker_board`
signature change. Drop the `build_up` signature change in favour of §5.6.

---

## 1. The power as we will implement it

Card text (BGA wording):

> At the end of your turn, you may place a whirlpool on any unoccupied space on the board.
> Whirlpool tokens built on or removed are returned to you. A worker cannot win by moving onto a
> whirlpool if the other whirlpool is on the board in an unoccupied space. Instead, the worker is
> forced to the other whirlpool and may win as if it moved up to that space.

Normalised into engine terms:

1. Charybdis owns exactly **two** whirlpool tokens. They start off-board ("on the card").
2. **End of Charybdis's turn only** (after her build), she *may* place one token from her supply on
   any unoccupied space (no worker, no dome). At most one placement per turn. She can never pick a
   placed token back up voluntarily.
3. A token returns to supply when its square's height changes — i.e. anyone building on it,
   doming it (Atlas), or removing a block from it (Ares). Both players' builds do this.
4. **Teleport (both players, always on):** if a worker *moves* onto whirlpool `W1`, and `W2` is on
   the board on an unoccupied space, the worker is forced to `W2`. Entry is a normal move and obeys
   all normal legality (height ±1, occupancy, god restrictions) **for `W1`**. The exit to `W2` has
   **no height restriction**.
5. The worker **cannot win on `W1`** (even at level 3 — it is forced off). It **wins on `W2`** if
   `W2` is level 3, "as if it had moved up", regardless of the entry height.
6. Forced movement does **not** trigger a whirlpool ("only if you move there, not if you are forced
   there") — Apollo swaps, Minotaur pushes, Charon pulls, Harpies slides, Odysseus flings, etc.

Sources: [BGA Charybdis doc](https://en.doc.boardgamearena.com/SantoriniPowerCharybdis),
[UltraBoardGames Golden Fleece](https://www.ultraboardgames.com/santorini/golden-fleece.php).

### 1a. The unifying height rule (settled by the BGA matchup notes)

The BGA page's per-god sections resolve what "counts as moving up" means, and they are consistent
with a single principle:

> **The teleport leg has no height delta.** Only the entry leg is a real move, and only the entry
> leg is subject to height restrictions. The one exception is the win check, where arriving at the
> exit is treated as "moved up".

Evidence, quoted from the page:

- **Athena** — "Charybdis may enter whirlpools without moving up, making Athena less effective.
  Charybdis is a strong counter." → the exit is *not* a climb. Athena still restricts the **entry**
  (you may not climb *into* the whirlpool while her flag is set).
- **Pan** — "Moving into whirlpools counts as moving up, negating his condition benefit." → a
  portal descent is never a Pan win. Combined with rule 5 (no win on the entry square), **Pan can
  never win via a whirlpool at all**.
- **Hypnus** — "Whirlpool wins bypass Hypnus's level-2 requirement." → direct confirmation that
  portal mates come from workers *not* on level 2, i.e. the §5.4 problem is real and known.
- **Zeus** — "Zeus can remove whirlpools by building beneath himself after teleporting."
- **Ares** — "If a block sits under whirlpools, Ares can remove both." → confirms §5.6's
  height-delta hook must fire on block *removal*, not just builds.
- **Limus** — "When adjacent to whirlpools, Limus controls them" → Limus's build ban is what stops
  Charybdis re-placing tokens; no special handling, it falls out.

Everything else follows: **Hades** restricts the entry only, **Persephone**'s must-climb is judged
on the entry only, **Hermes**' flat-move chain is judged on the entry only (so a portal leg neither
breaks nor extends his chain). The only place the height fiction applies is the win check.

Note this makes the *restriction* side of every god a non-event — which is exactly why the swap can
live below the height filters in `get_limited_moves_given_move_mask` (§5.3). All the difficulty
concentrates in win/mate detection (§5.4), as expected.

Also from the page: none of these matchups are rules-broken. Aphrodite, Apollo/Scylla, Ares,
Athena, Harpies, Hermes, Hypnus, Limus, Morpheus, Pan, Triton, Zeus, Graeae/Hydra/Proteus and the
double-builders are all listed as playable with a tier assessment (Charybdis is rated tier A). So
any `BANNED_MATCHUPS` entry we add is an **engine-scope** decision (`BannedReason::Engine`), not a
`BannedReason::Game` one, and each is a debt to pay down rather than a permanent ruling.

### Why this is the invasive one

Every other opponent-facing god in this engine is **subtractive**: Athena removes a climb, Limus
removes builds, Hypnus removes a worker, Europa's Talus adds a blocker mask, Hera shrinks the win
mask. They all fold into an existing mask in `get_generator_prelude_state`, and no god's move
generator needs to know they exist.

Charybdis is the first god that **rewrites the mapping from "the square you chose" to "the square
you end on"**, for the opponent, and simultaneously **adds win squares that are not reachable by
normal climbing rules**. The only precedent is Harpies (`slide_position` inside
`get_worker_end_move_state`), and Harpies is already the god with the most special-case escapes in
the codebase.

---

## 2. The central design decision: encode outcomes, not paths

**Decision D1 — a move's `move_to_position` is the square the worker *ends* on (the exit whirlpool),
not the entry whirlpool.**

This follows the same rule already used for multi-step powers in this codebase: encode
from/to/build and treat path reachability as a move-generation problem.

Consequences, all good:

- No opponent move struct changes. `MortalMove`, `DemeterMove`, … stay bit-identical; their
  `make_move` already does `worker_xor(from ^ to)` and lands the worker in the right place.
- No duplicate moves. If both whirlpools are reachable, "enter W1, exit W2" and "enter W2, exit W1"
  are two distinct outcomes, generated once each. `validate_non_duplicates` stays happy.
- The "cannot win on the entry whirlpool" rule becomes automatic: the entry square is never an
  outcome, so it can never be tagged as a win.

Cost: the entry square is no longer recoverable from the move alone, which matters in exactly two
places — `move_to_actions` (the UI wants to animate move → whirlpool → exit) and
`get_blocker_board` (blocking the entry blocks the win). Both are handled by recomputing the entry
from the board (§5.5, §5.7): given `from` and `to`, the entry is `to`'s partner whirlpool iff
`to` is a whirlpool and `from` is not adjacent-and-legal to `to`… which is ambiguous when both
hold. Simpler and exact: **the entry is the partner whirlpool whenever `to` is a whirlpool with an
on-board free partner** — because in that situation landing on `to` is *only* possible via the
partner (a direct move onto `to` would have teleported away).

### The swap trick and its correctness

Let `E` = the normally-legal destination set for one worker, `P = {W1, W2}` the on-board tokens.
Compute occupancy *excluding the moving worker's own start square* (it is vacated before the
teleport resolves).

```
if |P| == 2 && both unoccupied && |E ∩ P| == 1  →  outcomes = E ^ P
else                                            →  outcomes = E
```

- `|E ∩ P| == 0`: no whirlpool entered, nothing changes. (Note plain `E ^ P` would be **wrong**
  here — it would add both whirlpools as destinations. The `count_ones() == 1` guard is load-bearing.)
- `|E ∩ P| == 1`: entering the one reachable whirlpool lands you on the other. Exactly the swap.
- `|E ∩ P| == 2`: enter W1 → land W2, enter W2 → land W1. Outcome *set* is unchanged.
- One whirlpool occupied: it is not a legal destination, and it is not a legal exit either
  (partner not "unoccupied"), so no teleport is possible. Unchanged.

This is the old branch's `put_moves_through_portals`, and it is correct — but only if applied to a
destination set that was computed with the normal height rules, and only if the occupancy test is
per-moving-worker.

---

## 3. Interesting decisions (need a call before coding)

| # | Question | Options | Recommendation |
|---|---|---|---|
| ~~**D2**~~ | ~~Does exiting a whirlpool count as "moving up" for **Athena**?~~ | — | **RESOLVED** by §1a: no. Athena restricts the **entry** move only; a portal exit is not a climb. Zero extra code — the swap sits below the climb filter. |
| ~~**D3**~~ | ~~Does a portal exit that drops 2+ levels win for **Pan**?~~ | — | **RESOLVED** by §1a: no. "Moving into whirlpools counts as moving up, negating his condition benefit." Combined with "cannot win on the entry", Pan can *never* win via a whirlpool. Pan's `MATE_ONLY` fast path stays sound, but his win generator must still be excluded from the portal-mate widening in §5.4 — a Pan-style down-2 win is not available at the exit. |
| ~~**D4**~~ | ~~Does a portal exit violate **Hades** ("cannot move down")?~~ | — | **RESOLVED** by §1a: entry only. `validate_hades_moves` compares start/end heights, so it needs a Charybdis escape hatch (or better: teach it to recognise the portal and compare against the *entry* height). |
| ~~**D5**~~ | ~~Does entering a portal satisfy **Persephone**'s "must move up if able"?~~ | — | **RESOLVED** by §1a: entry only. `MUST_CLIMB` filters `E` *before* the swap, so this falls out for free. |
| **D6** | **Aphrodite**'s adjacency: measured on entry or outcome? | (a) Outcome (final position must neighbour her worker). (b) Entry. | **(a) Outcome** — the rule is about where workers end up. The swap must therefore be applied *before* `restrict_moves_by_affinity_area`, unlike Persephone. This asymmetry is a real trap; write the test. |
| **D7** | Worker standing on `W1` moves onto `W2` → teleported back to `W1`, i.e. ends where it started. Legal? | (a) Yes — it is a legal "move" that ends in place, effectively move-in-place + build. (b) Suppress it as degenerate. | **(a) Yes.** It follows from the rules, the engine already tolerates end-where-you-started moves (see the Proteus/Harpies escape in `validate_non_duplicates`), and `worker_xor(from ^ to) == worker_xor(0)` is a natural no-op. But it is a *tempo* resource (a free pass with a build) — deliberately include it in tests, and make sure it doesn't collide with `NULL_MOVE_DATA` or trip repetition/zugzwang logic. |
| **D8** | Can Charybdis place a token on a square she just built up this turn? | (a) Yes if it is not a dome. (b) No. | **(a)**. "Unoccupied" only excludes workers and domes. Ordering inside `make_move` matters: build first, then placement, then the "height changed → clear token" sweep must not eat the token she just placed. |
| **D9** | Token placement in `MATE_ONLY` generation | (a) Skip entirely (wins end the turn). (b) Generate. | **(a) Skip.** A winning move ends the game; the token is irrelevant. Already what the old branch does by returning early. |
| **D10** | Token placement in `INTERACT_WITH_KEY_SQUARES` (blocking) generation | (a) Skip. (b) Generate placements onto key squares. | **(b), mandatory.** Dropping a whirlpool onto the opponent's level-3 winning square *denies the win* (they get teleported off it). This is a genuine and non-obvious blocking resource; omitting it makes the search miss forced defences and mis-score positions as lost. |
| **D11** | Branching factor control for Charybdis's own turn | (a) Full cross product move × build × 25 token squares. (b) Restrict candidate token squares. | **(a) for correctness first**, then measure. See §6 — this is the main performance risk, and unsound pruning here will show up as consistency-checker failures. |
| **D12** | Scope: how many opponent gods are portal-correct at first merge? | (a) All 58. (b) Simple movers only, ban the rest. | **(b)** — phased, see §4. Banning is cheap and reversible (`BANNED_MATCHUPS`), shipping a subtly-wrong Apollo/Hermes interaction is not. |
| **D13** | UI action type | (a) Reuse `PartialAction::SetTalusPosition`. (b) New `PartialAction::PlaceWhirlpool(Square)`. | **(b)**. Talus is a dome-like blocker with completely different semantics and rendering; reuse would mislead the UI and the web app. Costs an enum variant in `gods.rs`, `pretty_board.rs`, `ui/src/main.rs`, and `web_app/src/common/api.tsx` + `action_selector.tsx` + `GameGridCanvas.tsx`. |

---

## 4. Phased scope for opponent gods

Portal handling has to be audited per god family. Proposal: land phase 1 with everything else in
`BANNED_MATCHUPS` (`BannedReason::Engine`), then lift bans a family at a time, each with its own
fuzzer run.

**Phase 1 — simple movers (portal applied once, inside the shared helper).**
Everything that gets its destinations from `get_basic_moves*` / `get_worker_next_move_state` and
loops `for worker_end_pos in worker_moves` — mortal, pan, atlas, hephaestus, demeter, prometheus,
athena, hades, hera, limus, hypnus, morpheus, hestia, clio, bia, maenads, zeus, chronus, nike,
polyphemus, graeae, medusa, eros, selene, iris, asteria, aeolus, hippolyta, europa, persephone,
aphrodite, poseidon, ares (move part), terpsichore. ~34 gods, free once the helper applies the swap
(§5.3), plus the mate-detection fix (§5.4).

**Phase 2 — displacement gods.** Apollo, Minotaur, Charon (+`charon_v2`), Scylla, Nemesis, Jason,
Achilles, Theseus, Odysseus, Medusa's kill, Bia's kill, Apollo V2. Rule 6 means the *displaced*
worker never teleports; the *mover* still can. Order of resolution matters and must be pinned:
- Apollo swaps onto a whirlpool: does Apollo teleport (leaving the swapped worker on Apollo's old
  square), or is the swap "forced movement" for both? Apollo *chose* to move there → he teleports.
  But then the exchanged worker is on a square Apollo no longer occupies, and the entry whirlpool is
  now empty — a three-square outcome that no existing move struct can encode. **Likely ban.**
- Minotaur pushes a worker off a whirlpool square and lands on it → Minotaur teleports, the pushed
  worker stays pushed. Encodable, but `get_blocker_board` and the push legality check both need the
  portal.

**Phase 3 — multi-step movers.** Artemis, Hermes, Triton, Pegasus, Castor, Proteus, Bellerophon,
Stymphalians. The portal must apply at *each* step, which turns "reachable set" into a graph
traversal with a teleport edge. Specific traps:
- **Hermes** — "if your workers do not move up or down". §1a resolves the rule (the teleport leg
  has no height delta, so a portal leg neither breaks nor extends his chain — only the entry leg is
  measured), and BGA lists the matchup as played. So this is *engine* difficulty only: his
  destination set is a flood-fill, and the portal adds a teleport edge to it, so the fill has to be
  re-run from the exit square. Ban in phase 1, unban in phase 3 with a real fix.
- **Artemis** — "not back to its initial space": if step 1 goes through the portal and step 2
  returns, is the forbidden square the start or the entry?
- **Triton** — perimeter chaining; a portal exit into the interior ends the chain.

**Phase 4 / long-lived bans.** Harpies — the slide is forced movement, so by rule 6 a worker slid
onto a whirlpool does *not* teleport, but a worker that slides *off* a whirlpool square, or slides
onto the entry and stops there, needs an ordering decision (slide first, then teleport?). BGA lists
the matchup as played and even favourable for Charybdis ("forces dome-building, which harms
Harpies"), so this is engine debt, not a rules hole — but it interacts with
`get_worker_end_move_state`, the one place Charybdis and Harpies both want to rewrite the
destination. Also Hydra (worker count), and Hecate if ever added (banned on BGA).
- Chronus should be fine. whirlpools and talus squares cannot overlap

---

## 5. Implementation, file by file

### 5.1 Registration (mechanical)
- `GodName::Charybdis = 58` (next after `Triton = 57`), `pub(crate) mod charybdis;`,
  `charybdis::build_charybdis()` appended to `ALL_GODS_BY_ID` at index 58, added to `WIP_GODS`.
- `descriptions.rs` — the `match self` is exhaustive; add the card text.
- `god_name_to_nnue_size` — **leave at 0** for now (see §5.9).

### 5.2 State representation
`god_data[charybdis_player]` = a 25-bit bitboard of on-board whirlpools. Supply count is
`2 - popcount`, so no counter field is needed. Port from the branch:
- `parse_god_data` / `stringify_god_data` — comma-separated square list; round-trips through FEN.
- `pretty_stringify_god_data` — "Whirlpools at C3, D4".
- `flip_horizontal` / `flip_vertical` / `flip_transpose` — bitboard flips (needed for symmetry
  canonicalisation in datagen/matchups).
- `_get_token_mask` + `with_get_token_mask_fn` + `is_token_user` on `GodPower`.
- All mutation goes through `xor_god_data` so the Zobrist hash stays correct. `validate_hash` in the
  consistency checker will catch any path that forgets.

### 5.3 The opponent-side chokepoint
Add to `GeneratorPreludeState`:
```rust
pub portal_squares: BitBoard,   // both tokens, only if exactly 2 are on board; else EMPTY
```
computed as `other_god.get_token_mask(board, !player)` when `other_god.is_token_user`.

Add to `WorkerStartMoveState`:
```rust
pub active_portal: BitBoard,    // portal_squares, or EMPTY if either end is occupied
                                // by a worker other than this one
```
i.e. `portal_squares & !(all_workers_and_frozen_mask ^ worker_start_mask)` then zeroed unless
`count_ones() == 2`.

Then apply the swap **inside** `get_limited_moves_given_move_mask` (the single function all the
`get_basic_moves*` variants funnel through) so all ~34 phase-1 gods inherit it — but *after* the
`MUST_CLIMB` filter (D5) and *before* the affinity restriction (D6). That ordering constraint is
the fiddly part: affinity is currently applied inside the same function. Concretely:

```
E = neighbours & height-legal & !blockers        // MUST_CLIMB already folded in here
E = put_moves_through_portals(E, active_portal)  // D5 before, D6 after
E = restrict_moves_by_affinity_area(...)
```

Verified against `move_helpers.rs:742` — `get_limited_moves_given_move_mask<MUST_CLIMB,
APPLY_AFFINITY>` is the single funnel, and it has **two** exit paths: the `MUST_CLIMB` branch
(Persephone) returns early and never applies affinity, and the normal branch applies affinity last.
The swap must be inserted into **both** branches, immediately before the affinity call. Note the
`blockers` argument still contains the moving worker's own square (that is what stops it moving
onto itself), which is why `active_portal` needs its own `^ worker_start_mask` occupancy test rather
than reusing `blockers`.

Gods that build their destination set by hand (`iris`, `pegasus`, `bellerophon`, `triton`,
`artemis`, `urania`, `hermes`, `stymphalians`, `demeter`, `hephaestus` — the `basic=0` column of the
audit) do **not** route through this and must be handled individually or banned in phase 1.

### 5.4 Mate detection — the one place that needs real work

This is the part that cannot be hidden inside a bitboard helper, and the BGA page confirms it is a
real game mechanic rather than a corner case ("Whirlpool wins bypass Hypnus's level-2 requirement").

Every generator narrows mate search on the assumption that **only a worker standing on level 2 can
win next move**. Concretely, three hardcoded forms:

| Form | Sites | Purpose |
|---|---|---|
| `let checkable_mask = prelude.exactly_level_2;` | **44** | feeds `modify_prelude_for_checking_workers` (MATE_ONLY acting-worker filter) and `other_threatening_workers` (check tagging) |
| `is_mate_only::<F>() \|\| worker_start_state.worker_start_height == 2` | **56** | guards win extraction per worker |
| `get_basic_acting_workers::<F>` → `acting_workers &= prelude.exactly_level_2` | 1 (shared) | same as row 1, for gods that don't use `checkable_mask` |

Against an armed portal with a level-3 exit, a worker on level **0** that can step onto the entry
whirlpool has a forced mate. All three forms silently drop it.

#### The reusable concept: `mate_start_mask`

Rather than teaching 44 generators about whirlpools, generalise the thing they are all approximating:

> **`mate_start_mask`** = the set of squares from which a worker could win on its *next* move,
> given the current board. Normally exactly the level-2 squares. Against an armed portal whose exit
> is at level 3, additionally the squares from which the *entry* whirlpool is enterable.

Computed once in `get_generator_prelude_state`:

```rust
// move_helpers.rs
pub mate_start_mask: BitBoard,   // == exactly_level_2 in every non-Charybdis game

fn portal_mate_sources(prelude_parts, portal: BitBoard, win_mask, neighbor_map) -> BitBoard {
    let mut res = BitBoard::EMPTY;
    for exit in portal & exactly_level_3 & win_mask {
        let entry = portal ^ exit.to_board();
        // squares from which `entry` is a legal destination — over-approximating is safe
        res |= neighbor_map[entry.lowest_square()] & enterable_heights(entry);
    }
    res
}
```

Then `mate_start_mask = exactly_level_2 | portal_mate_sources(..)`, and add a matching per-worker
convenience so the 56 height guards become a field lookup rather than arithmetic:

```rust
// WorkerStartMoveState
pub can_mate: bool,     // (prelude.mate_start_mask & worker_start_mask).is_not_empty()
```

The per-god diff is then **two pure substitutions**, no new logic anywhere:

```diff
-    let checkable_mask = prelude.exactly_level_2;
+    let checkable_mask = prelude.mate_start_mask;
     modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);
...
-        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
+        if is_mate_only::<F>() || worker_start_state.can_mate {
```

Properties that make this safe to roll out across 44 files:

- **No-op without Charybdis.** `mate_start_mask == exactly_level_2` bit-for-bit in every existing
  matchup, so `visit_tester` node counts must be *identical*. Assert that in a test — it turns a
  100-file sweep into a mechanically verifiable refactor.
- **Over-approximation is free.** Widening `mate_start_mask` only widens which workers are
  *considered*; the actual win emission stays exact, because
  `moves_to_level_3 = worker_moves & exactly_level_3 & win_mask` runs on the **post-swap outcome
  set** — so `W2`-at-level-3 is emitted as a win and `W1`-at-level-3 is not, which is rule 5 for
  free. If `portal_mate_sources` is sloppy about heights or neighbour maps (Urania's wrapping,
  Aeolus's wind, Hippolyta's diagonals), the cost is wasted work, never a wrong move. Start with
  the crude version (`portal armed && exit at level 3 → all own workers`) and tighten later.
- **The portal exit can't change mid-turn.** Building the exit whirlpool from level 2 to 3 *removes
  the token* (§5.6), so a whirlpool can only be a mate exit if it is *already* at level 3 before
  the move. `mate_start_mask` therefore depends only on the pre-move board and needs no post-build
  adjustment — which is what lets it live in the prelude.

#### Gods that need more than the substitution

- **Pan** (§1a/D3): a portal exit is "moving up", so Pan gets no down-2 win at the exit and no win
  at the entry. Pan's generator must *not* widen — leave his down-2 scan on `exactly_level_2`
  semantics and let only the standard level-3 arrival benefit from the swap.
- **Hypnus**: `get_reach_board_when_can_be_level_3` and `get_standard_reach_board_from_parts` gate
  threats on `other_threatening_workers.count_ones() + is_now_lvl_2 >= 2` — the "he can only freeze
  one worker" heuristic. A portal mate comes from a *low* worker, which Hypnus's freeze (highest
  worker) usually cannot touch, so the heuristic under-counts and would drop real threats. Simplest
  correct move: skip the Hypnus narrowing entirely when the portal is armed with a level-3 exit.
- **Bellerophon / Achilles / Chronus / Hermes / Triton** and friends use non-standard checkable
  masks (`exactly_level_2 | exactly_level_1`, `worker_start_height == 1`, …). These are the
  stragglers in the 44/56 counts — handle by hand, and note that phase 1 bans most of them anyway.

### 5.5 Check / reach boards

`get_standard_reach_board*` computes next-turn threat squares as
`neighbours(end_pos) & win_mask & unblocked`, gated by `is_now_lvl_2`. Two matching changes, both
reusable:

1. Replace the `is_now_lvl_2: u32` multiplier on `WorkerEndMoveState` with
   `is_mate_capable: u32` = `(worker_end_mask & prelude.mate_start_mask).is_not_empty() as u32`.
   Same substitution property as §5.4: identical to the old value without Charybdis.
2. Apply the portal to `next_turn_moves` inside the reach helpers, so a worker ending adjacent to
   the entry whirlpool has `W2` in its threat set.

Caveat on (2): the exact swap (`|E ∩ P| == 1 → E ^ P`) is only valid **per source square**. Reach
boards union several sources (`other_threatening_neighbors`), where the guard breaks. Use a
deliberate over-approximation for reach boards only:

```rust
if (reach & portal).is_not_empty() { reach | portal } else { reach }
```

Over-approximating threats costs a wasted check extension; under-approximating loses a forced
defence. `search.rs` already tolerates and backtracks on "claimed to be in check but wasn't", so
over-approximation is the safe side. Keep the exact swap for actual move generation (§5.3), where
the source square is always a single worker.

### 5.6 Builds clear tokens — central hook
Do **not** re-plumb `build_up(square, player, other_god)` the way the old branch did (it touches
every god). Instead, wrap at `GodPower::make_move`:

```rust
pub fn make_move(&self, board, other_god, action) {
    let token_owner = /* self or other_god, if is_token_user */;
    let before = board.height_map;            // 4 × u32, cheap
    (self._make_move)(board, other_god, action);
    if let Some(p) = token_owner {
        let changed = fold_xor(before, board.height_map) & MAIN_SECTION_MASK;
        let stale = BitBoard(board.god_data[p as usize]) & changed;
        if stale.is_not_empty() { board.xor_god_data(p, stale.0); }
    }
}
```

This covers every builder, every multi-build god (Hephaestus, Demeter, Poseidon, Hestia), Atlas's
domes, **and Ares removing a block** (height change in either direction) — matching the BGA rulings
on Zeus and Ares. It also covers Charybdis's own build.

Ordering vs D8: Charybdis's `make_move` must place her token *after* her build, and the sweep must
run on the height delta only, so a token placed on a square she just built survives. If that proves
awkward, place the token before the build inside `make_move` and let the sweep remove it — but that
changes the legal-placement set, so prefer the former and test it.

Paths that bypass `GodPower::make_move` and need the same treatment:
`pretty_board.rs`'s action replay (it calls `board.build_up` directly) and any UI/wasm apply path.

### 5.7 Blocker boards / key squares
Port the branch's `get_blocker_board(self, board, player)` signature change (~58 mechanical impls +
2 call sites in `search.rs`). For a win that lands on `W2` via `W1`, the blocker board must include
**both** whirlpool squares plus the from/to pair: the defender can block by occupying the entry,
occupying the exit, **or building on either token** (which returns it to supply and disarms the
portal). That last one is a blocking mechanism no other god has, and it only works because of §5.6.

Symmetrically, `INTERACT_WITH_KEY_SQUARES` generation for Charybdis must include token placements
onto key squares (D10).

### 5.8 Charybdis's own move generator
Signature and skeleton as per CLAUDE.md; the branch's version is a fine starting point but:
- It never applies the portal to her *own* mate detection (same §5.4 issue, her side).
- Its token-placement candidate set is `unblocked_squares & !exactly_level_3&build_mask & MAIN &
  !tokens`, which is *narrower than the rules* — placement is "any unoccupied space on the board",
  not just squares near the worker. Fix: `!(all_workers | domes | existing_tokens) &
  MAIN_SECTION_MASK`. (The branch's `unblocked_squares` is neighbour-agnostic, so this may already
  be right — verify.)
- `is_check` is commented out; restore it via `get_standard_reach_board` + the portal.

### 5.9 NNUE
Per the new-god pipeline: `with_nnue_god_name(GodName::Mortal)` so Charybdis evaluates as Mortal
initially, and leave `god_name_to_nnue_size(Charybdis) = 0` so
`TOTAL_GOD_DATA_FEATURE_COUNT_FOR_NNUE` stays at 125 and the `_ASSERTION` const holds. When a
retrain happens, give her 25 features (bitboard fan-out, exactly like Clio's coins in
`emit_god_data_features`) and bump the constant per the comment protocol in `gods.rs`. Datagen
scores for Charybdis positions will be garbage until then — do not blend them into training data.

### 5.10 UI / web
New `PartialAction::PlaceWhirlpool(Square)` (D13) wired through `gods.rs`, `pretty_board.rs`,
`ui/src/main.rs` (colour + label), and the web app's `api.tsx` / `action_selector.tsx` /
`GameGridCanvas.tsx`. Whirlpool squares also need a board-render treatment in `pretty_board.rs` so
FEN dumps in test failures are readable.

---

## 6. Performance risk

Charybdis's branching factor is `moves × builds × (1 + free_squares)`. With ~15 free squares
mid-game that is a **~16× node multiplier** on her turns versus Mortal. Mitigations, in order:

1. Wins and mate-only generation skip token placement entirely (D9).
2. Order the no-token move first; token placements are quiet moves and should sort below killers so
   LMR prunes them hard.
3. If still too slow, measure before restricting. Candidate sound-ish restrictions to *evaluate*
   (each changes the game, so gate behind a matchup A/B): only place on level-3 squares, squares
   adjacent to any worker, or squares adjacent to the partner token.
4. `tree_perf` and `visit_tester` before/after on a Charybdis position set.

Second risk: §5.4's widened `mate_start_mask` disables the level-2 mate filter for *both* sides in
portal positions. That is only when a level-3 whirlpool is armed, but it makes those nodes
noticeably more expensive.

---

## 7. Edge cases to test

Each of these deserves a FEN-based unit test in `charybdis.rs` (and, where it involves the opponent,
a consistency-checker assertion).

**Portal basics**
1. One token on board → no teleport, whirlpool square behaves as a normal square.
2. Two tokens, worker moves onto `W1` from level 0, `W2` at level 3 → **win**, even though the
   worker never climbed. (The §5.4 case.)
3. Two tokens, `W1` at level 3, `W2` at level 0, opponent worker at level 2 adjacent to `W1` →
   **no win**; the move is legal and lands them at level 0. A level-3 whirlpool is a *trap*.
4. `W1` at level 3, `W2` at level 3 → win.
5. `W2` occupied by a worker (either player's) → no teleport; the mover may sit on `W1`.
6. Both whirlpools reachable by the same worker → exactly two moves generated (land on `W1`, land on
   `W2`), no duplicates, no missing move.
7. Worker standing on `W1` moves to `W2` → ends on `W1` (D7). Assert `from == to`, assert the build
   is generated from the start square, assert no hash/NNUE drift.
8. Entry whirlpool is 2+ levels above the worker → not a legal entry, no teleport.
9. Neither whirlpool adjacent to any worker → generation identical to Mortal.

**Token lifecycle**
10. Opponent builds on a whirlpool → token returns to supply; Charybdis may replace it next turn.
11. Atlas domes a whirlpool square → token returns.
12. Ares removes a block under a whirlpool → token returns (BGA-confirmed).
13. Zeus builds under himself while standing on a whirlpool → token returns (BGA-confirmed).
14. Hephaestus/Poseidon/Demeter multi-build hitting a whirlpool once → removed once, no double-xor.
15. Prometheus builds on a whirlpool **before** moving → the portal must be disarmed for his own
    move that same turn. (Ordering bug magnet.)
16. Charybdis builds on her own whirlpool and places a new token the same turn (D8).
17. Charybdis places her second token → the portal arms immediately, and the **opponent** may use it
    on the very next turn.
18. Token placement onto a square that is level 3 → legal (denial play).
19. Token placement onto an occupied square / a dome / the other whirlpool → **not** generated.
20. Both tokens on board → no placement moves generated at all.

**Search / blocking**
21. Opponent threatens a portal mate; Charybdis blocks by **building on either whirlpool**. Assert
    the blocker board includes both squares and that `INTERACT_WITH_KEY_SQUARES` generates it.
22. Opponent threatens a normal level-3 mate; Charybdis blocks by **placing a whirlpool on the
    winning square** (D10). Assert the blocking move is generated.
23. Charybdis places a whirlpool that *hands the opponent* a mate — assert the search sees it and
    avoids it (i.e. that the opponent's mate generation finds portal mates).
24. Check detection: worker adjacent to `W1` with `W2` at level 2 that gets built to 3 → is it
    tagged as a check? (§5.5.)

**Cross-god (per phase)**
25. Athena flag set, portal exit is higher → **legal** (D2/§1a). Same position but the *entry* is a
    climb → **illegal**. Both directions matter; only the second is Athena's business.
26. Pan + portal exit 2 levels down → **no win** (D3/§1a). Also Pan entering a level-3 whirlpool
    from level 2 → no win (rule 5). Pan can never win through a portal; assert both.
27. Hades + portal exit lower → **legal** (D4/§1a) — and the checker's `validate_hades_moves`
    must compare against the *entry* height, not the exit.
28. Persephone forcing a climb where the only climb is a portal entry → the entry counts (D5).
    Also: portal entry is flat but the exit is higher → does **not** satisfy Persephone.
29. Aphrodite adjacency measured on the exit square (D6) — the ordering asymmetry with D5.
30. Hypnus + a level-3 portal exit → a level-0 worker's mate must be found, and Hypnus's
    "freeze the highest worker" narrowing must not suppress it (BGA: "whirlpool wins bypass
    Hypnus's level-2 requirement").
30b. **Regression guard:** `mate_start_mask == exactly_level_2` and
    `is_mate_capable == is_now_lvl_2` for every non-Charybdis matchup, asserted directly, plus
    identical `visit_tester` node counts before/after the §5.4 sweep.
31. Europa's Talus on/next to a whirlpool; frozen mask vs portal occupancy.
32. Urania wrapping neighbours into a whirlpool.
33. Hera: portal exit onto a perimeter level 3 → not a win.

**Symmetry / serialisation**
34. FEN round-trip with 0, 1, 2 tokens.
35. All three flips (h/v/transpose) preserve token positions.
36. Zobrist hash matches `compute_hash_from_scratch` after every token mutation
    (`validate_hash` covers this if the fuzzer reaches those states).

---

## 8. Verification strategy

1. **New consistency-checker validators** (`validate_charybdis_moves`), run on every fuzzer position:
   - No generated move ever *ends* on a whirlpool whose partner is on-board and unoccupied.
   - No winning move ends on such a whirlpool.
   - Token count ≤ 2 always; tokens never coincide with a worker or a dome.
   - Token positions change by at most one bit per turn, and only Charybdis's own turn may *add* a
     token; opponent turns may only remove one.
   - Every whirlpool square whose height changed this move has its token cleared.
2. **Fuzzer runs per phase**: `cargo run -p santorini_core --bin fuzzer -r -- -g charybdis -s -t 300`
   plus `-G <opponent>` sweeps over each phase's unbanned god list. Note the known blind spot: the
   fuzzer under-explores level-3-worker displacement paths, so hand-write FENs for cases 2–4.
3. **Perft-style spot checks**: hand-count legal moves for 3–4 small positions with the portal armed.
4. **`visit_tester` / `tree_perf`** before and after §5.4's `mate_start_mask` change to quantify the
   node-count cost on non-Charybdis matchups (it must be **zero** — the mask defaults to
   `exactly_level_2` and only widens against an armed portal; assert that in a test).
5. **Battler**: Charybdis vs Mortal at fixed nodes to sanity-check that she isn't accidentally
   winning/losing 100% (a common symptom of a mis-signed win rule), then the usual matchup sweep.

---

## 9. Suggested commit sequence

1. `charybdis: god data, FEN, flips, registration` — state only, no move gen, god unusable but
   parseable. (Tests: FEN round-trip, flips.)
2. `gods: thread token mask through the prelude` — `_get_token_mask`, `portal_squares`,
   `WorkerStartMoveState.active_portal`. No behaviour change yet.
3. `gods: builds return whirlpool tokens` — §5.6 central hook + checker validator. Verifiable
   against a hand-placed god_data even before move gen exists.
4. `gods: route destinations through whirlpools` — §5.3 swap in the shared helper. Phase-1 gods
   become portal-correct. Fuzz.
5. `gods: portal mates` — §5.4 `mate_start_mask`. Fuzz + `visit_tester` regression check.
6. `charybdis: move generation` — §5.8, including token placement and D10 blocking placements.
7. `gods: blocker boards include portal squares` — §5.7 signature change.
8. `charybdis: reach boards and check tagging` — §5.5.
9. `charybdis: ban unaudited matchups` — `BANNED_MATCHUPS` entries for phases 2–4, with reasons.
10. `charybdis: UI action + rendering` — §5.10.
11. Later, per family: `charybdis: unban <family>` with its own fuzz evidence.

---

## 10. Open questions for you

- ~~**D2/D3/D4/D5**~~ — resolved from the BGA per-god notes, see §1a. The teleport leg has no
  height delta for any restriction; the height fiction applies to the win check only.
- **D7** (end-where-you-started as a free tempo move) — confirm you want it, since it interacts with
  whatever zugzwang assumptions the eval carries.
- **Phase-1 god list** — is banning ~24 gods at first merge acceptable, or do you want Apollo /
  Artemis / Minotaur in the first cut?
- **Placement-phase question**: whirlpools are placed "at the end of your turn"; I am assuming
  worker-placement turns don't count, so the board starts with zero tokens and the first can appear
  at the end of Charybdis's first real turn. Confirm.
    -> yes.
