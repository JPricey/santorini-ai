import { type GameState, Player, type PlayerGameState, type PlayerType, Square, squareStrToSquare, type SquareType } from "./game_state";
import { Result, Ok, Err } from "ts-results";

function _parseHeights(heightString: string): Result<Array<number>, string> {
    heightString = heightString.trim();
    let res: Array<number> = [];

    for (let i = 0; i < heightString.length; i++) {
        const char = heightString[i];
        if (char >= '0' && char <= '9') {
            res.push(parseInt(char, 10));
        }
    }

    if (res.length !== 25) {
        return Err(`Invalid fen string: height map must be 25 characters, but was ${res.length}`)
    }

    return Ok(res);
}

function _parsePlayer(player: string): Result<PlayerType, string> {
    player = player.trim();
    if (player === '1') {
        return Ok(Player.One);
    } else if (player === '2') {
        return Ok(Player.Two);
    } else {
        return Err(`Invalid player in fen string: ${player}`);
    }

}

type GodNameSection = {
    godName: string,
    optionalSection: string,
    tokens: Array<SquareType>,
};

function _parseGodNameSection(godSection: string): Result<GodNameSection, string> {
    const regex = /([^\[]*)(\[(.*)\])?/;
    const match = godSection.match(regex);
    if (!match) {
        return Err(`Invalid god section in fen string: ${godSection}`);
    }
    const godName = match[1].trim();
    const optionalSection = match[3] ? match[3].trim() : '';
    const tokens: Array<SquareType> = [];
    if (godName == 'europa') {
        if (Square.hasOwnProperty(optionalSection)) {
            tokens.push(Square[optionalSection as keyof typeof Square]);
        }
    }

    return Ok({
        godName: godName,
        optionalSection: optionalSection,
        tokens: tokens,
    });
}

function _parsePlayerSection(playerSection: string): Result<PlayerGameState, string> {
    const isWin = playerSection.includes("#");
    playerSection = playerSection.replaceAll('#', '');

    const parts = playerSection.split(':');
    if (parts.length > 2) {
        return Err('Invalid player section in fen string: too many colons');
    }

    const godSection = _parseGodNameSection(parts[0]);
    if (godSection.err) {
        return godSection;
    }

    const god = godSection.val.godName;
    const workers: Array<SquareType> = [];
    if (parts.length === 2) {
        const workerStrings = parts[1].split(',');
        for (const workerCoord of workerStrings) {
            const workerSquare = workerCoord.trim();
            const squareResult = squareStrToSquare(workerSquare);
            if (squareResult.err) {
                return Err(`Invalid worker square in fen string: ${workerSquare}`);
            }
            workers.push(squareResult.val);
        }
    }

    workers.sort((a, b) => a - b);;

    return Ok({
        god: god,
        workers: workers,
        tokens: godSection.val.tokens,
        isWin: isWin,
        otherAttributes: godSection.val.optionalSection,
    });
}

export function parseFen(fen: string): Result<GameState, string> {
    const splits = fen.split('/');
    if (splits.length !== 4) {
        return Err('Invalid fen string: wrong number of / segments')
    }
    const parsedHeight = _parseHeights(splits[0]);
    if (parsedHeight.err) {
        return parsedHeight;
    }
    const parsedPlayer = _parsePlayer(splits[1]);
    if (parsedPlayer.err) {
        return parsedPlayer;
    }
    const player1Result = _parsePlayerSection(splits[2]);
    if (player1Result.err) {
        return player1Result;
    }
    const player2Result = _parsePlayerSection(splits[3]);
    if (player2Result.err) {
        return player2Result;
    }
    const gameState: GameState = {
        heights: parsedHeight.val,
        players: [
            player1Result.val,
            player2Result.val,
        ],
        currentPlayer: parsedPlayer.val,
    };
    return Ok(gameState);
}
