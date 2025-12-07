import csv
from dataclasses import dataclass
from collections import defaultdict, Counter

@dataclass
class Matchup:
    gods: tuple[str, str]
    winner: int
    winning_god: str
    losing_god: str
    moves: int

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
    matchups = read_matchups("tmp/all_matchups.csv")
    all_gods = get_all_gods(matchups)
    print(all_gods)
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

    print('Most wins:')
    for (k, v) in gods_by_wins.most_common():
        print(k, v, "/", gods_by_losses[k])

    SHOW_LOSSES_GODS = [
        "Charon",
        "Limus",
        "Aeolus",
        "Apollo",
        "Limus",
        "Scylla",
    ]

    SHOW_WINS_GODS = [
        "Pan",
        "Selene",
        "Pegasus",
    ]

    for god in SHOW_WINS_GODS:
        print(f"{god} wins")
        for m in non_mirrors:
            if m.winning_god == god:
                print(m)

    for god in SHOW_LOSSES_GODS:
        print(f"{god} losses")
        for m in non_mirrors:
            if m.losing_god == god:
                print(m)

main()
