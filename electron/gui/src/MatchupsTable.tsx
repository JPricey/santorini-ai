import { parseMatchupsString } from "./common/gods";

function sigmoid(x: number) {
    return 1 / (1 + Math.exp(-x));
}

export function MatchupsTable({ selectedGods }: { selectedGods?: string[] }) {
    const matchups = parseMatchupsString();
    const sortedMatchups = matchups.sort((a, b) => a.absoluteScore - b.absoluteScore);

    const relevantMatchups = sortedMatchups.filter((matchup) => {
        return !selectedGods || selectedGods.length === 0 ||
            (selectedGods.includes(matchup.god1) && selectedGods.includes(matchup.god2));
    });

    return (
        <table>
            <thead>
                <tr>
                    <th>First</th>
                    <th>Second</th>
                    <th>Score</th>
                    <th>Win %</th>
                </tr>
            </thead>
            <tbody>
                {relevantMatchups.map((matchup, index) => (
                    <tr key={index}>
                        <td>{matchup.god1}</td>
                        <td>{matchup.god2}</td>
                        <td>{matchup.score.toFixed(2)}</td>
                        <td>{(sigmoid(matchup.score / 400) * 100).toFixed(2)}%</td>
                    </tr>
                ))}
            </tbody>
        </table>
    );
}
