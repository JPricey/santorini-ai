import { expect, test } from 'vitest'
import { parseFen } from "./fen";
import { Square, squareStrToSquare, type GameState } from "./game_state";

test('squareStrToSquare', () => {
    expect(squareStrToSquare('A5').val).toBe(Square.A5);
    expect(squareStrToSquare('B1').val).toBe(Square.B1);

    expect(squareStrToSquare('0').val).toBe(0);
    expect(squareStrToSquare('10').val).toBe(10);

    expect(squareStrToSquare('E6').err).toBe(true);
});

test('parseFen', () => {
    const result = parseFen('00000 11111 00000 00000 00000/1/mortal:A1, B2/athena:C3, C4#');
    expect(result.ok).toBe(true);
    const resultValue = result.val as GameState;

    expect(resultValue.heights).toEqual([0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    expect(resultValue.players[0].isWin).toBe(false);
    expect(resultValue.players[0].workers).toEqual([Square.B2, Square.A1]);
    expect(resultValue.players[0].god).toBe('mortal');

    expect(resultValue.players[1].isWin).toBe(true);
});

test('parseAthenaFen', () => {
    const result = parseFen('00000 11111 00000 00000 00000/1/mortal:A1,B2/athena[^]:C3,C4');
    expect(result.ok).toBe(true);
    const resultValue = result.val as GameState;

    expect(resultValue.players[1].god).toBe("athena");
    expect(resultValue.players[1].otherAttributes).toBe("^");
});
