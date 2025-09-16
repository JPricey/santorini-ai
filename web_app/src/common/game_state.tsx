import { Err, Ok, Result } from "ts-results";
import { assertUnreachable } from "./utils";

export const God = {
    Mortal: 'mortal',
    Pan: 'pan',
    Artemis: 'artemis',
    Hephaestus: 'hephaestus',
    Atlas: 'atlas',
    Athena: 'athena',
    Minotaur: 'minotaur',
    Hermes: 'hermes',
    Demeter: 'demeter',
    Apollo: 'apollo',
    Prometheus: 'prometheus',
    Urania: 'urania',
    Graeae: 'graeae',
    Hera: 'hera',
    Limus: 'limus',
    Hypnus: 'hypnus',
    Harpies: 'harpies',
    Aphrodite: 'aphrodite',
    Persephone: 'persephone',
    Hades: 'hades',
    Morpheus: 'morpheus',
    Aeolus: 'aeolus',
    Hestia: 'hestia',
} as const;
export type GodType = typeof God[keyof typeof God];

export const WIP_GODS: Set<GodType> = new Set([
    God.Aphrodite,
    God.Persephone,
    God.Hades,
    God.Morpheus,
    God.Aeolus,
    God.Hestia,
]);

export const Square = {
    A5: 0,
    B5: 1,
    C5: 2,
    D5: 3,
    E5: 4,
    A4: 5,
    B4: 6,
    C4: 7,
    D4: 8,
    E4: 9,
    A3: 10,
    B3: 11,
    C3: 12,
    D3: 13,
    E3: 14,
    A2: 15,
    B2: 16,
    C2: 17,
    D2: 18,
    E2: 19,
    A1: 20,
    B1: 21,
    C1: 22,
    D1: 23,
    E1: 24,
} as const;
export type SquareType = typeof Square[keyof typeof Square];

export const Player = {
    One: 0,
    Two: 1,
} as const;
export type PlayerType = typeof Player[keyof typeof Player];

export function playerToPrettyColorString(player: PlayerType): string {
    switch (player) {
        case Player.One:
            return 'White';
        case Player.Two:
            return 'Black';
        default:
            return assertUnreachable(player);
    }
}

export function playerToString(player: PlayerType): string {
    switch (player) {
        case Player.One:
            return 'One';
        case Player.Two:
            return 'Two';
        default:
            return assertUnreachable(player);
    }
}

export const Direction = {
    NW: "NW",
    N: "N",
    NE: "NE",
    E: "E",
    SE: "SE",
    S: "S",
    SW: "SW",
    W: "W",
} as const;
export type DirectionType = typeof Direction[keyof typeof Direction];

export type Coord = {
    row: number,
    col: number,
}

export type PlayerGameState = {
    god: string,
    workers: Array<SquareType>,
    isWin: boolean,
    otherAttributes: string,
};

export type GameState = {
    heights: Array<number>,
    players: Array<PlayerGameState>,
    currentPlayer: PlayerType,
};

export function getWinner(gameState: GameState): PlayerType | null {
    if (gameState.players[0].isWin) {
        return Player.One;
    }
    if (gameState.players[1].isWin) {
        return Player.Two;
    }
    return null;
}

export function isGameOver(gameState: GameState) {
    return gameState.players[0].isWin || gameState.players[1].isWin;
}

export function getPlayerOnSquare(gameState: GameState, square: SquareType): PlayerType | null {
    if (gameState.players[0].workers.includes(square)) {
        return Player.One;
    }
    if (gameState.players[1].workers.includes(square)) {
        return Player.Two;
    }
    return null;
}

export function squareToCoord(square: SquareType): Coord {
    return {
        row: 4 - Math.floor(square / 5),
        col: square % 5,
    };
}

export function coordToSquare(coord: Coord): SquareType {
    if (coord.row < 0 || coord.row > 4 || coord.col < 0 || coord.col > 4) {
        throw new Error("Invalid coordinates");
    }
    return ((4 - coord.row) * 5) + coord.col as SquareType;
}

export function squareToSquareStr(square: SquareType): string {
    const coord = squareToCoord(square);
    return `${String.fromCharCode('A'.charCodeAt(0) + coord.col)}${coord.row + 1}`;
}

export const SQUARE_LOOKUP: { [key: string]: SquareType } = {};
export const SQUARE_NUMBER_STRING_LOOKUP: { [key: string]: SquareType } = {};
for (let i = 0; i < 25; i++) {
    SQUARE_LOOKUP[squareToSquareStr(i as SquareType)] = i as SquareType;
    SQUARE_NUMBER_STRING_LOOKUP[`${i}`] = i as SquareType;
}

export function squareStrToSquare(squareStr: string): Result<SquareType, string> {
    squareStr = squareStr.trim().toUpperCase();
    if (SQUARE_LOOKUP.hasOwnProperty(squareStr)) {
        return Ok(SQUARE_LOOKUP[squareStr]);
    }
    if (SQUARE_NUMBER_STRING_LOOKUP.hasOwnProperty(squareStr)) {
        return Ok(SQUARE_NUMBER_STRING_LOOKUP[squareStr]);
    }
    return Err(`Invalid square string: ${squareStr}`);
}
