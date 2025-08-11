const MATCHUPS_STRING = `
hephaestus, pan, 1805.25
apollo, hermes, 1795.00
hephaestus, atlas, 1794.25
atlas, athena, 1674.00
apollo, pan, 1586.25
prometheus, atlas, 1527.00
prometheus, pan, 1456.50
apollo, artemis, 1279.00
apollo, atlas, 1278.75
prometheus, demeter, 1241.25
prometheus, hermes, 1212.25
demeter, pan, 1186.50
athena, pan, 1155.00
minotaur, pan, 1102.25
artemis, pan, 1046.50
apollo, demeter, 1028.75
hephaestus, hermes, 937.00
hephaestus, artemis, 906.50
hephaestus, demeter, 897.50
hephaestus, minotaur, 847.50
athena, demeter, 832.50
apollo, prometheus, 829.25
hermes, pan, 802.50
prometheus, artemis, 786.75
artemis, demeter, 777.75
demeter, atlas, 755.00
demeter, hermes, 674.00
apollo, minotaur, 625.25
minotaur, hermes, 585.25
apollo, hephaestus, 561.25
hermes, atlas, 533.00
apollo, athena, 456.75
prometheus, athena, 441.00
atlas, artemis, 411.00
prometheus, minotaur, 382.25
athena, hephaestus, 312.00
athena, minotaur, 215.25
atlas, pan, 180.50
athena, hermes, 163.50
artemis, hermes, 157.25
artemis, athena, 129.50
hephaestus, prometheus, 127.75
prometheus, hephaestus, 122.50
minotaur, artemis, 104.50
minotaur, atlas, 96.25
demeter, minotaur, 48.75
minotaur, demeter, 37.50
atlas, minotaur, 37.00
hermes, athena, 1.25
athena, artemis, -19.50
pan, atlas, -47.00
minotaur, athena, -83.75
artemis, minotaur, -95.25
hermes, artemis, -117.00
hephaestus, athena, -129.75
minotaur, prometheus, -272.25
artemis, atlas, -310.50
athena, prometheus, -323.25
athena, apollo, -440.50
atlas, demeter, -462.00
demeter, prometheus, -483.50
pan, demeter, -487.50
atlas, hermes, -497.00
hephaestus, apollo, -510.25
hermes, minotaur, -548.00
demeter, hephaestus, -549.50
demeter, athena, -557.25
hermes, demeter, -563.25
minotaur, apollo, -605.00
minotaur, hephaestus, -724.00
artemis, prometheus, -745.00
demeter, artemis, -746.00
pan, hermes, -776.50
prometheus, apollo, -789.00
artemis, hephaestus, -920.00
pan, athena, -933.25
demeter, apollo, -948.50
hermes, hephaestus, -952.50
pan, minotaur, -984.25
pan, artemis, -1011.25
hermes, prometheus, -1118.75
atlas, apollo, -1196.00
artemis, apollo, -1221.25
pan, prometheus, -1223.25
atlas, prometheus, -1447.00
pan, apollo, -1524.50
pan, hephaestus, -1555.25
athena, atlas, -1556.75
atlas, hephaestus, -1713.25
hermes, apollo, -1798.25
`;

type MatchupFairness = {
    god1: string;
    god2: string;
    score: number;
    absoluteScore: number;
};

let _cachedMatchups: Array<MatchupFairness> | null = null;

export function parseMatchupsString(): Array<MatchupFairness> {
    if (_cachedMatchups) {
        return _cachedMatchups;
    }

    const lines = MATCHUPS_STRING.trim().split('\n');
    const matchups: Array<MatchupFairness> = [];
    for (const line of lines) {
        const [god1, god2, scoreStr] = line.split(',').map(s => s.trim());
        const score = parseFloat(scoreStr);
        const absoluteScore = Math.abs(score);
        matchups.push({ god1, god2, score, absoluteScore });
    }

    _cachedMatchups = matchups;
    return matchups;
}

let _cachedGodNames: Array<string> | null = null;
export function getAllGodNames(): Array<string> {
    if (_cachedGodNames) {
        return _cachedGodNames;
    }

    const matchups = parseMatchupsString();
    const godNamesSet = new Set<string>();
    for (const matchup of matchups) {
        godNamesSet.add(matchup.god1);
        godNamesSet.add(matchup.god2);
    }

    const godNames = Array.from(godNamesSet).sort();
    _cachedGodNames = godNames;
    return godNames;
}
