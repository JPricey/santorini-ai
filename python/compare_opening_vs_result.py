"""Compare the opening balance screen against who actually won the game.

Both inputs key on ordered (god1, god2) with god1 seated as player one, and
mean_eval is already player-one-relative, so a positive eval and a "One" result
agree. Prints a calibration table - how often the opening called the winner, by
eval magnitude - and every disagreement, largest opening edge first.
"""

import csv
from collections import Counter, defaultdict
from dataclasses import dataclass

EVALS_FILE = "data/matchup_evals.csv"
GAMES_FILE = "tmp/all_matchups_10s.csv"
# GAMES_FILE = 'tmp/all_matchups.csv'

OUT_CSV = "tmp/opening_vs_result.csv"
OUT_DISAGREEMENTS = "tmp/opening_vs_result_disagreements.txt"

EVAL_BANDS = [(0, 250), (250, 500), (500, 1000), (1000, 2000), (2000, 9000), (9000, 99999)]


@dataclass
class Comparison:
    god1: str
    god2: str
    mean_eval: float
    final_eval: float
    mean_depth: float
    is_decisive: bool
    player_one_won: bool

    @property
    def favoured(self) -> str:
        return self.god1 if self.mean_eval > 0 else self.god2

    @property
    def agrees(self) -> bool:
        return (self.mean_eval > 0) == self.player_one_won


def load_comparisons() -> list[Comparison]:
    screen = {(r["god1"], r["god2"]): r for r in csv.DictReader(open(EVALS_FILE))}

    results = defaultdict(Counter)
    for row in csv.DictReader(open(GAMES_FILE)):
        results[(row["god1"], row["god2"])][row["winning_player"]] += 1

    comparisons = []
    for key, tally in results.items():
        row = screen.get(key)
        # A tie has no winner to disagree with, and a zero eval favours nobody.
        if row is None or tally["One"] == tally["Two"] or float(row["mean_eval"]) == 0:
            continue
        comparisons.append(
            Comparison(
                god1=key[0],
                god2=key[1],
                mean_eval=float(row["mean_eval"]),
                final_eval=float(row["final_eval"]),
                mean_depth=float(row["mean_depth_reached"]),
                is_decisive=row["is_decisive"] == "true",
                player_one_won=tally["One"] > tally["Two"],
            )
        )

    comparisons.sort(key=lambda c: -abs(c.mean_eval))
    return comparisons


def print_calibration(comparisons: list[Comparison]) -> None:
    agreed = sum(1 for c in comparisons if c.agrees)
    total = len(comparisons)
    print(f"{total} matchups in both files")
    print(f"agree {agreed} ({100 * agreed / total:.1f}%), disagree {total - agreed}")
    print("\nHow often the opening called the winner (50% would be a coin flip):")
    for low, high in EVAL_BANDS:
        band = [c for c in comparisons if low <= abs(c.mean_eval) < high]
        if band:
            right = sum(1 for c in band if c.agrees)
            print(f"  |mean eval| {low:5d}-{high:<5d} n={len(band):4d}  {100 * right / len(band):5.1f}%")


def write_csv(comparisons: list[Comparison]) -> None:
    with open(OUT_CSV, "w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            ["god1", "god2", "mean_eval", "final_eval", "abs_mean", "favoured",
             "winner", "agrees", "is_decisive", "mean_depth"]
        )
        for c in comparisons:
            writer.writerow(
                [c.god1, c.god2, f"{c.mean_eval:.0f}", f"{c.final_eval:.0f}",
                 f"{abs(c.mean_eval):.0f}", c.favoured,
                 "One" if c.player_one_won else "Two",
                 c.agrees, c.is_decisive, f"{c.mean_depth:.1f}"]
            )


def write_disagreements(comparisons: list[Comparison]) -> int:
    disagreements = [c for c in comparisons if not c.agrees]
    with open(OUT_DISAGREEMENTS, "w") as handle:
        handle.write(
            f"{len(disagreements)} of {len(comparisons)} matchups where the opening "
            f"favoured one side and the other won, largest opening edge first\n\n"
        )
        handle.write(f"{'matchup':34s} {'mean':>7} {'final':>7} {'winner':>7} {'depth':>6}  favoured\n")
        for c in disagreements:
            flag = "  [DECISIVE]" if c.is_decisive else ""
            handle.write(
                f"{c.god1 + ' v ' + c.god2:34s} {c.mean_eval:+7.0f} {c.final_eval:+7.0f} "
                f"{'One' if c.player_one_won else 'Two':>7} {c.mean_depth:6.1f}  {c.favoured}{flag}\n"
            )
    return len(disagreements)


def main() -> None:
    comparisons = load_comparisons()
    print_calibration(comparisons)
    write_csv(comparisons)
    count = write_disagreements(comparisons)
    print(f"\nwrote {OUT_CSV} ({len(comparisons)} rows)")
    print(f"wrote {OUT_DISAGREEMENTS} ({count} rows)")


if __name__ == "__main__":
    main()
