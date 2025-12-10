import csv
from dataclasses import dataclass
from collections import defaultdict, Counter
from textwrap import fill

@dataclass
class Matchup:
    gods: tuple[str, str]
    winner: int
    winning_god: str
    losing_god: str
    moves: int

# MATCHUPS_FILE = 'tmp/all_matchups.csv'
# MATCHUPS_FILE = 'tmp/all_matchups_10s.csv'
MATCHUPS_FILE = 'tmp/pegasus_v2_matchups.csv'

def read_matchups(csv_path):
    matchups = []

    with open(csv_path, newline='') as csvfile:
        reader = csv.reader(csvfile)
        next(reader)  # Skip header
        for row in reader:
            [god1, _, god2, _, winner, moves] = row
            if winner == "One":
                winner = 0
            elif winner == "Two":
                winner = 1
            else:
                raise ValueError(f"Unexpected winner value: {winner}")
            gods = (god1, god2)

            matchups.append(Matchup(
                gods=(god1, god2),
                winner=winner,
                winning_god=gods[winner],
                losing_god=gods[1-winner],
                moves=int(moves)
            ))

    return matchups

def get_all_gods(data):
    p1 = {m.gods[0] for m in data}
    p2 = {m.gods[1] for m in data}
    return list(sorted(p1.union(p2)))

def fill_counter(counter, all_gods):
    for g in all_gods:
        counter[g] += 0
    return counter

def main():
    matchups = read_matchups(MATCHUPS_FILE)
    all_gods = get_all_gods(matchups)

    non_mirrors = [m for m in matchups if m.gods[0] != m.gods[1]]
    gods_by_wins = fill_counter(Counter(m.winning_god for m in non_mirrors), all_gods)
    gods_by_losses = fill_counter(Counter(m.losing_god for m in non_mirrors), all_gods)
    matchups_with_gods = defaultdict(int)

    longest_games = list(sorted(matchups, key = lambda x: -x.moves))
    for m in longest_games[0:10]:
        print(m)

    for m in matchups:
        matchups_with_gods[m.gods[0]] += 1
        matchups_with_gods[m.gods[1]] += 1

    for (k, v) in gods_by_wins.most_common():
        print(k, v, "/", gods_by_losses[k])
    ordered_gods = [p[0] for p in gods_by_wins.most_common()]
    matchup_chart = dict()
    for m in non_mirrors:
        matchup_chart[m.gods] = m.winning_god

    SHOW_LOSSES_GODS = [
        "Charon",
        "Limus",
        "Aeolus",
        "Apollo",
        "Limus",
        "Scylla",
    ]

    SHOW_WINS_GODS = [
        "Pegasus",
        "Pan",
        "Selene",
    ]

    for god in SHOW_LOSSES_GODS:
        print(f"{god} losses")
        for m in non_mirrors:
            if m.losing_god == god:
                print(m)

    for god in SHOW_WINS_GODS:
        print(f"{god} wins")
        for m in non_mirrors:
            if m.winning_god == god:
                print(m)

    output_matchup_csv()

def output_matchup_csv():
    matchups = read_matchups(MATCHUPS_FILE)
    all_gods = get_all_gods(matchups)
    get_god_wins = fill_counter(Counter(m.winning_god for m in matchups), all_gods)
    get_god_present_in_matches = defaultdict(int)
    for m in matchups:
        get_god_present_in_matches[m.gods[0]] += 1
        get_god_present_in_matches[m.gods[1]] += 1

    god_winrates = [(get_god_wins[god] / get_god_present_in_matches[god], god) for god in get_god_present_in_matches.keys()]
    ordered_god_names = list(god for _, god in sorted(god_winrates, key=lambda x: -x[0]))

    matchup_chart = dict()
    for m in matchups:
        matchup_chart[m.gods] = m.winner

    god_stats = dict()
    for god in ordered_god_names:
        first_games = sum(1 for m in matchups if m.gods[0] == god)
        first_wins = sum(1 for m in matchups if m.gods[0] == god and m.winner == 0)
        second_games = sum(1 for m in matchups if m.gods[1] == god)
        second_wins = sum(1 for m in matchups if m.gods[1] == god and m.winner == 1)
        total_games = sum(1 for m in matchups if god in m.gods)
        total_wins = sum(1 for m in matchups if m.winning_god == god)
        god_stats[god] = (first_wins, first_games, second_wins, second_games, total_wins, total_games)

    with open("tmp/matchup_chart.csv", "w", newline="") as f:
        writer = csv.writer(f)
        enumerated_god_names = [f"{i + 1}: {g}" for (i, g) in enumerate(ordered_god_names)]
        writer.writerow([""] + enumerated_god_names + ["First", "Second", "Total"])
        for i, god1 in enumerate(ordered_god_names):
            row = [f"{i + 1}: {god1}"]
            for j, god2 in enumerate(ordered_god_names):
                winner = matchup_chart.get((god1, god2))
                if winner == 0:
                    row.append("1")
                elif winner == 1:
                    row.append("2")
                else:
                    row.append("-")
            first_wins, first_games, second_wins, second_games, total_wins, total_games = god_stats[god1]
            row.append(f"{first_wins}/{first_games}")
            row.append(f"{second_wins}/{second_games}")
            row.append(f"{total_wins}/{total_games}")

            writer.writerow(row)

main()
