# Experiment log — 2026-07-12

Chronological. Every experiment gets an entry when its result lands, including
rejects.

**Metrics:**
- Node screen: `visit_tester` fixed-depth suites (mortal -r4, hermes -r20,
  prometheus -r10), deterministic. Only big/consistent effects are readable —
  micro ordering tweaks produce ±20% per-suite chaos.
- Game test: `sprt` bin, color-swapped pairs @ 0.5s/move, elo0=0 elo1=10;
  W-D-L = candidate sweeps / splits / baseline sweeps. Mirror set ≈ 46 pairs,
  resolution roughly ±60 elo.

**Protocol (afternoon onward):** each test on branch `test/<slug>`,
implemented by an Opus subagent in a git worktree; positive results merge to
main. ⚠ Agent worktrees can spawn from a STALE base — every brief must verify
`git rev-parse HEAD` before branching (see cutoff-maluses v1).

## Scoreboard

| Ver | Change | Nodes (sum) | Game result | Status |
|---|---|---|---|---|
| v123 | history malus fix (+improving reorder) | mixed | −16 lean vs v122 | shipped (bug fix) |
| v124 | response/follow rotation fix | all improve | +33 lean vs v122 | shipped (bug fix) |
| v125 | history persistence + publish guard | identical | +40 vs v124; **+61 cumulative vs v122** | shipped |
| v126 | NMP gate >3→>2 | −8% | even vs v125 | shipped (node win) |
| v127 | RFP margin 100+80d | −18% | **+93 vs v126** (p≈0.014) | shipped |
| v128 | RFP 75+60d | n/a | −38 vs v127 | reverted |
| v129 | cutoff maluses −15·nd | −1.6% | **+85 vs v127** (p≈0.01) | merged e3519e8 |
| v130 | drop per-ply history table | −11% | +46 vs v129 | merged 15a6afc |
| v131 | global history cap 1024→8192 | ? | running | branch test/history-caps |

Rejected without version: 2-killer slots, fail-low bonus gating, malus
scale-up ×2/×8, lazy NNUE update, TT depth-preferred single slot, parity-pair
accumulators, aspiration windows (all below).

Node baseline trajectory (3-suite sum, same depths): pre-fix 20.9M → v127
14.9M → v130 13.0M (−38% total).

---

## Morning: inline experiments (pre-branch protocol)

### v123: history malus fix (+ improving branch-order fix)
Change: maluses were applied to the loop counter instead of the move hash
(slots 1..336 polluted; real moves never penalized). One-word fix. Also made
the ply-4 `improving` branch reachable (Joe's edit).
Nodes vs v122: mortal −7.6%, hermes −14.8%, prometheus +15.7%.
SPRT vs v122 (64 pairs, all-matchup order): 10-41-13, elo ~−16, llr −0.32.
Inconclusive; no regression. (Explained later by the rotation bug still
drowning the signal.)

### v124: response/follow history rotation fix
Change: read path rotated the move hash by 4 (`HALF_USIZE` = byte-width/2
bug), write path by 32 → the two largest tables (±8192 caps) were never read
back, returning noise that dominated move ordering. Single
`HISTORY_HASH_ROTATION = usize::BITS/2` both paths.
Nodes vs v122: mortal −8.9%, hermes −5.8%, prometheus −3.8% (all improved).
SPRT vs v122: first attempt aborted by an sprt-tool LLR bug (zero variance on
all-draw start exploded the ratio → spurious instant H0; fixed with ½ virtual
W/D/L regularization + regression test). Re-run: **21-28-15, elo ~+33,
llr +0.26** — positive lean, 58% of decisive pairs.

### 2-killer slots + fail-low bonus gating: REJECTED
Nodes vs v124: killers-only mortal −1.4% / hermes −9.2% / prometheus +40%;
combined +6.5% / −8.4% / +24%. Per-suite chaos, sums mildly negative.
Lesson: node screen is only readable for big/consistent effects.

### v125: game-long history persistence (+ root publish guard)
Change: `Histories` moved into `TranspositionTable` — persists across
searches like the TT, cleared together by `tt.reset()`.
Nodes: byte-identical (visit_tester resets per scenario, by design).
SPRT vs v124 (mirrors, 52 pairs): **18-22-12, elo ~+40, llr +0.27**.
Cumulative v125 vs v122 (mirrors, 46 pairs): **14-26-6, elo ~+61, llr +0.51**
(70% of decisive pairs, p≈0.06).

### Malus magnitude scale-up ×2/×8: REJECTED
Nodes vs v125 non-monotonic (prometheus +45..48% both ways) — noise-dominated
knob; kept −4·nd (later superseded by v129's scheme change).

### Lazy NNUE accumulator update: REJECTED
Skip `replace_from_state` at TT-eval nodes: tree byte-identical but ~48%
SLOWER — diff-accumulator cost scales with feature distance between
consecutive evals; staying synced along the path is the cheap mode. Comment
left in search.rs.

### TT depth-preferred + aging (single slot): REJECTED
mortal +5.1% nodes on the deep suite — recency beats depth for a single slot.
Kept write-only `age` field + `new_search()` as infra for future buckets.

### Parity-pair NNUE accumulators: REJECTED
Tree byte-identical; interleaved tree_perf A/B showed no effect above machine
noise (wall clock drifted ±40% during the session — absolute timings across
the session are NOT comparable; always interleave).

### `improving` ply-2 vs ply-4: ply-4-primary CONFIRMED
ply-2 (chess convention) cost +19.5% (mortal) / +27% (prometheus). Joe's
ply-4-primary kept — two full rounds smooths Santorini's per-round eval
oscillation feeding `improving`/`eval_delta`→LMR.

### v126: NMP depth gate >3 → >2 (SHIPPED)
Nodes vs v125: mortal −2.2%, hermes −18.2%, prometheus −4.6% (all-suite
improvement). `>1` regressed mortal. NMP eval-condition variant (`eval≥beta`)
was inert (±0.3%).
SPRT vs v125 (mirrors): 9-28-9, even — kept for the node savings.

### v127: slimmer RFP margin (SHIPPED); v128 slimmer still (REVERTED)
v127 margin `100+80·d−80·improving`: nodes −12..−23% all suites; SPRT vs
v126: **19-20-7, elo ~+93, llr +0.65** (p≈0.014) — best single change of the
session. v128 `75+60·d`: **6-29-11, elo ~−38** — overpruned, reverted. The
margin curve peaks near v127.

### Aspiration windows: REJECTED
Naive ±35, parity-aware (window vs same-parity score 2 iterations back —
evals oscillate with depth parity), ±150, fail-to-full: all cost nodes
(best still mortal +5.9% / prometheus +25%). Suspected interaction with
always-replace single-slot TT; revisit after TT buckets. The root publish
guard (don't report fail-low scores as best moves) was kept — verified
behavior-neutral.

---

## Afternoon: branch protocol (Opus subagents, worktrees)

Main progression: 9b8aae5/011983a/5718d4f (v127 baseline commits) → e3519e8
(v129) → 8d4aa15 (db) → 15a6afc (v130) → 12e632d (cleanup).

### branch test/cutoff-maluses (v1): INVALID — wrong base
Agent worktree silently branched from 30f2cb1 (pre-fix main); its "+48%
nodes" verdict compared buggy-base+change against fixed-main baselines.
Meaningless. Led to the mandatory STEP-0 base verification in every brief.

### branch test/cutoff-maluses-v2 (−50·nd): REJECTED at node screen
Base 5718d4f verified, commit aa6570c. Nodes: mortal **−15.5%**, hermes
**+45.3%**, prometheus +13.6% — hermes breached the 25% gate. The mortal gain
motivated the gentle retry.

### branch test/cutoff-maluses-gentle (v129): SHIPPED, merged e3519e8
Change: cutoff-time maluses at **−15·nd** to all tried moves at beta cutoff;
continuous per-move malus removed.
Nodes: mortal −8.7%, hermes +17.1%, prometheus +9.2%; sum −1.6%.
SPRT vs v127 (mirrors): **15-27-4, elo ~+85, llr +0.81** (79% of decisive
pairs, p≈0.01).

### branch test/no-ply-table (v130): SHIPPED, merged 15a6afc (+cleanup 12e632d)
Change: per-ply history table removed (nonstandard; absolute-ply keying
drifted meaning once histories persisted game-long). Cleanup commit removed
the dead field/allocation (~26MB)/`set_move_min`; counts byte-identical.
Nodes: mortal **−19.0%**, hermes +2.1%, prometheus +1.1%; sum −11.2%.
SPRT vs v129 (mirrors): **13-26-7, elo ~+46, llr +0.36**.
New baselines @ 12e632d: mortal 7,357,035 · hermes 2,605,415 · prometheus
3,032,947 (sum 12,995,397); SPRT baseline v130.

### branch test/history-caps (v131): STOPPED mid-test (by request)
Change: `GLOBAL_MOVE_HISTORY_MAX` 1024 → 8192 (equal vote with
response/follow). Passed the node screen; SPRT vs v130 interrupted at 24/46
pairs: **5-12-7, mild negative lean** — early read says equal weighting isn't
obviously right. v131 binary + versions.txt line exist; branch change
uncommitted (lives in the agent worktree only). Resume or rerun at will.
