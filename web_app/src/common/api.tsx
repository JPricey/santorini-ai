import { get_next_moves_interactive, get_banned_matchups, get_pretty_game_state } from "../../pkg/wasm_app";
import { type GameState, type DirectionType } from "./game_state";
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
    SetWindDirection: 'set_wind_direction',
    SetTalusPosition: 'set_talus_position',
    Build: 'build',
    Destroy: 'destroy',
    Dome: 'dome',
    EndTurn: 'end_turn',
    NoMoves: 'no_moves',
} as const;
export type PlayerActionType = typeof PlayerActionTypes[keyof typeof PlayerActionTypes];

export type MoveWorkerData = {
    dest: string;
    meta?: {
        type: 'move_enemy_worker'
        value: {
            from: string,
            to: string,
        }
    } | {
        type: 'kill_enemy_worker'
        value: {
            square: string,
        }
    }
};

export type PlayerAction =
    | { type: typeof PlayerActionTypes.PlaceWorker; value: string }
    | { type: typeof PlayerActionTypes.SelectWorker; value: string }
    | { type: typeof PlayerActionTypes.MoveWorker; value: MoveWorkerData }
    | { type: typeof PlayerActionTypes.Build; value: string }
    | { type: typeof PlayerActionTypes.Destroy; value: string }
    | { type: typeof PlayerActionTypes.Dome; value: string }
    | { type: typeof PlayerActionTypes.SetTalusPosition; value: string }
    | { type: typeof PlayerActionTypes.SetWindDirection; value: DirectionType | null }
    | { type: typeof PlayerActionTypes.EndTurn }
    | { type: typeof PlayerActionTypes.NoMoves };

export function getNextMoves(fen: string): NextMoves {
    return get_next_moves_interactive(fen);
}

export function getBannedMatchups(): Set<string> {
    return new Set(get_banned_matchups());
}

export function getPrettyGameStateFromFen(fen: string): GameState {
    return get_pretty_game_state({ fen: fen })
}

export function getPrettyGameStateWithActions(fen: string, actions: Array<PlayerAction>): GameState {
    return get_pretty_game_state({ fen: fen, actions: actions });
}

export function describeActionType(actionType: PlayerActionType): string {
    switch (actionType) {
        case PlayerActionTypes.PlaceWorker:
            return `Place Worker`;
        case PlayerActionTypes.SelectWorker:
            return `Select Worker`;
        case PlayerActionTypes.MoveWorker:
            return `Move Worker`;
        case PlayerActionTypes.Build:
            return `Build`;
        case PlayerActionTypes.Dome:
            return `Dome`;
        case PlayerActionTypes.Destroy:
            return `Destroy`;
        case PlayerActionTypes.SetTalusPosition:
            return `Place Talus`;
        case PlayerActionTypes.EndTurn:
            return `End Turn`;
        case PlayerActionTypes.NoMoves:
            return `No Moves`;
        case PlayerActionTypes.SetWindDirection:
            return `Set Wind Direction`;
        default:
            return assertUnreachable(actionType);
    }
}

function moveDesc(data: MoveWorkerData): string {
    let base = data.dest;

    if (data.meta === undefined) {
        return base;
    }

    switch (data.meta.type) {
        case 'move_enemy_worker': {
            base += ` ${data.meta.value.from}→${data.meta.value.to})`;
            break;
        }
        case 'kill_enemy_worker': {
            base += ` x${data.meta.value.square})`;
            break;
        }
        default:
            return assertUnreachable(data.meta);
    }

    return `${base}`;
}

export function describeAction(action: PlayerAction): string {
    switch (action.type) {
        case PlayerActionTypes.PlaceWorker:
        case PlayerActionTypes.SelectWorker:
        case PlayerActionTypes.Build:
        case PlayerActionTypes.Dome:
        case PlayerActionTypes.Destroy:
        case PlayerActionTypes.SetTalusPosition:
            return `${describeActionType(action.type)} (${action.value})`;
        case PlayerActionTypes.MoveWorker:
            return `${describeActionType(action.type)} (${moveDesc(action.value)})`;
        case PlayerActionTypes.SetWindDirection:
            return `${describeActionType(action.type)} (${action.value})`;
        case PlayerActionTypes.EndTurn:
        case PlayerActionTypes.NoMoves:
            return describeActionType(action.type);
        default:
            return assertUnreachable(action);
    }
}

