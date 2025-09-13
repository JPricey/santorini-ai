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
    pass

def main():
    matchups = read_matchups("tmp/all_matchups.csv")
    non_mirrors = [m for m in matchups if m.gods[0] != m.gods[1]]
    gods_by_wins = Counter(m.winning_god for m in non_mirrors)
    gods_by_losses = Counter(m.losing_god for m in non_mirrors)
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

    print("Aeolus losses")
    for m in non_mirrors:
        if m.losing_god == "Aeolus":
            print(m)

    print("Apollos losses")
    for m in non_mirrors:
        if m.losing_god == "Apollo":
            print(m)

    print("Limus losses")
    for m in non_mirrors:
        if m.losing_god == "Limus":
            print(m)

    print("Pans wins")
    for m in non_mirrors:
        if m.winning_god == "Pan":
            print(m)

main()
