import { get_next_moves_interactive } from "../../pkg/wasm_app";
import { squareStrToSquare, type GameState, type SquareType } from "./game_state";
import { assertUnreachable } from "./utils";

export type NextMoves = {
    type: "next_moves",
    original_str: string,
    start_state: string,
    next_states: Array<NextState>,
}

export type NextState = {
    next_state: string,
    actions: Array<PlayerAction>,
};

export const PlayerActionTypes = {
    PlaceWorker: 'place_worker',
    SelectWorker: 'select_worker',
    MoveWorker: 'move_worker',
    MoveWorkerWithSwap: 'move_worker_with_swap',
    MoveWorkerWithPush: 'move_worker_with_push',
    Build: 'build',
    Dome: 'dome',
    EndTurn: 'end_turn',
} as const;
export type PlayerActionType = typeof PlayerActionTypes[keyof typeof PlayerActionTypes];

export type PlayerAction =
    | { type: typeof PlayerActionTypes.PlaceWorker; value: string }
    | { type: typeof PlayerActionTypes.SelectWorker; value: string }
    | { type: typeof PlayerActionTypes.MoveWorker; value: string }
    | { type: typeof PlayerActionTypes.MoveWorkerWithPush; value: [string, string] }
    | { type: typeof PlayerActionTypes.MoveWorkerWithSwap; value: string }
    | { type: typeof PlayerActionTypes.Build; value: string }
    | { type: typeof PlayerActionTypes.Dome; value: string }
    | { type: typeof PlayerActionTypes.EndTurn };

export function getNextMoves(fen: string): NextMoves {
    return get_next_moves_interactive(fen);
}

export function describeActionType(actionType: PlayerActionType): string {
    switch (actionType) {
        case PlayerActionTypes.PlaceWorker:
            return `Place Worker`;
        case PlayerActionTypes.SelectWorker:
            return `Select Worker`;
        case PlayerActionTypes.MoveWorker:
            return `Move Worker`;
        case PlayerActionTypes.MoveWorkerWithSwap:
            return `Swap Worker`;
        case PlayerActionTypes.MoveWorkerWithPush:
            return `Move Worker & Push`;
        case PlayerActionTypes.Build:
            return `Build`;
        case PlayerActionTypes.Dome:
            return `Dome`;
        case PlayerActionTypes.EndTurn:
            return `End Turn`;
        default:
            return assertUnreachable(actionType);
    }
}

export function describeAction(action: PlayerAction): string {
    switch (action.type) {
        case PlayerActionTypes.PlaceWorker:
        case PlayerActionTypes.SelectWorker:
        case PlayerActionTypes.MoveWorker:
        case PlayerActionTypes.MoveWorkerWithSwap:
        case PlayerActionTypes.Build:
        case PlayerActionTypes.Dome:
            return `${describeActionType(action.type)} (${action.value})`;
        case PlayerActionTypes.MoveWorkerWithPush:
            return `${describeActionType(action.type)} (${action.value[0]}>${action.value[1]}})`;
        case PlayerActionTypes.EndTurn:
            return describeActionType(action.type);
        default:
            return assertUnreachable(action);
    }
}

export function gameStateWithActions(gameState: GameState, partialActions: Array<PlayerAction>): GameState {
    const currentPlayerIdx: number = gameState.currentPlayer;
    const otherPlayerIdx = 1 - currentPlayerIdx;

    const result: GameState = JSON.parse(JSON.stringify(gameState));
    let selectedWorkerSquare: SquareType | null = null;
    for (const action of partialActions) {
        switch (action.type) {
            case PlayerActionTypes.PlaceWorker: {
                const square = squareStrToSquare(action.value);
                if (square.ok) {
                    result.players[currentPlayerIdx].workers.push(square.val);
                }
                break;
            }
            case PlayerActionTypes.SelectWorker: {
                const square = squareStrToSquare(action.value);
                if (square.ok) {
                    selectedWorkerSquare = square.val;
                }
                break;
            }
            case PlayerActionTypes.MoveWorker: {
                if (selectedWorkerSquare === null) {
                    break;
                }
                const fromIdx = result.players[currentPlayerIdx].workers.indexOf(selectedWorkerSquare);
                if (fromIdx === -1) {
                    break;
                }
                const toSquare = squareStrToSquare(action.value);
                if (!toSquare.ok) {
                    break;
                }
                result.players[currentPlayerIdx].workers[fromIdx] = toSquare.val;
                selectedWorkerSquare = null;
                break;
            }
            case PlayerActionTypes.MoveWorkerWithSwap: {
                if (selectedWorkerSquare === null) {
                    break;
                }
                const fromIdx = result.players[currentPlayerIdx].workers.indexOf(selectedWorkerSquare);
                if (fromIdx === -1) {
                    break;
                }
                const toSquare = squareStrToSquare(action.value);
                if (!toSquare.ok) {
                    break;
                }
                result.players[currentPlayerIdx].workers[fromIdx] = toSquare.val;

                const swapIdx = result.players[otherPlayerIdx].workers.indexOf(toSquare.val);
                if (swapIdx === -1) {
                    break;
                }
                result.players[otherPlayerIdx].workers[swapIdx] = selectedWorkerSquare;

                selectedWorkerSquare = null;
                break;
            }
            case PlayerActionTypes.MoveWorkerWithPush: {
                if (selectedWorkerSquare === null) break;
                const toSquare = squareStrToSquare(action.value[0]);
                if (!toSquare.ok) break;

                const fromIdx = result.players[currentPlayerIdx].workers.indexOf(selectedWorkerSquare);
                if (fromIdx === -1) break;
                result.players[currentPlayerIdx].workers[fromIdx] = toSquare.val;

                const pushSquare = squareStrToSquare(action.value[1]);
                if (!pushSquare.ok) break;
                const oppoIdx = result.players[otherPlayerIdx].workers.indexOf(toSquare.val);
                if (oppoIdx === -1) break;
                result.players[otherPlayerIdx].workers[toSquare.val] = pushSquare.val;

                selectedWorkerSquare = null;
                break;
            }
            case PlayerActionTypes.Build: {
                const buildSquare = squareStrToSquare(action.value);
                if (buildSquare.ok) {
                    const squareIdx = buildSquare.val;
                    result.heights[squareIdx] += 1;
                }
                break;
            }
            case PlayerActionTypes.Dome: {
                const buildSquare = squareStrToSquare(action.value);
                if (buildSquare.ok) {
                    const squareIdx = buildSquare.val;
                    result.heights[squareIdx] = 4;
                }
                break;
            }
            case PlayerActionTypes.EndTurn:
                break;
            default:
                assertUnreachable(action);
        }
    }

    return result;
}
