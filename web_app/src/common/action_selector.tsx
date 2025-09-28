import { getNextMoves, PlayerActionTypes, type NextState, type PlayerAction, type PlayerActionType } from "./api";
import { isGameOver, squareToSquareStr, type GameState, type SquareType, Square, type DirectionType } from "./game_state";
import { assertUnreachable, isListDeepContain, isListSubset } from "./utils";

export type ActionSelectorNextStep = {
    isDone: true,
    value: NextState,
} | {
    isDone: false,
    options: Array<PlayerAction>,
};

const DirectionToUISquareMap = {
    "NW": Square.E1,
    "N": Square.C1,
    "NE": Square.A1,
    "E": Square.A3,
    "SE": Square.A5,
    "S": Square.C5,
    "SW": Square.E5,
    "W": Square.E3,
} as const;

function maybeDirectionToSquare(direction: DirectionType | null): SquareType {
    if (direction === null) {
        return Square.C3;
    }

    return DirectionToUISquareMap[direction] as SquareType;
}

export function intersectionMatchType(square: SquareType, action: PlayerAction): PlayerActionType | null {
    const squareStr = squareToSquareStr(square);

    switch (action.type) {
        case PlayerActionTypes.PlaceWorker:
        case PlayerActionTypes.SelectWorker:
        case PlayerActionTypes.Build:
        case PlayerActionTypes.Destroy:
        case PlayerActionTypes.Dome:
        case PlayerActionTypes.SetTalusPosition:
            return action.value === squareStr ? action.type : null;
        case PlayerActionTypes.MoveWorker:
            return action.value.dest === squareStr ? action.type : null;
        case PlayerActionTypes.EndTurn:
            return null;
        case PlayerActionTypes.NoMoves:
            return action.type;
        case PlayerActionTypes.SetWindDirection:
            return maybeDirectionToSquare(action.value ?? null) === square ? action.type : null;
        default:
            assertUnreachable(action);
    }
}

export class ActionSelector {
    private readonly fen: string;
    private readonly gameState: GameState;
    private readonly allOutcomes: Array<NextState>;
    public readonly nextStep: ActionSelectorNextStep;
    public readonly selectedActions: Array<PlayerAction>;

    constructor(gameState: GameState, fen: string, selectedActions: Array<PlayerAction>) {
        this.fen = fen;
        this.gameState = gameState;
        this.selectedActions = selectedActions;

        if (isGameOver(gameState)) {
            this.allOutcomes = [];
        } else {
            const nextMoves = getNextMoves(fen);
            for (const nextState of nextMoves.next_states) {
                nextState.actions.push({ type: 'end_turn' });
            }
            this.allOutcomes = nextMoves.next_states;
        }

        this.nextStep = this._getNextStep();
    }

    getSelectorForNextAction(nextAction: PlayerAction): ActionSelector {
        const newActions = [...this.selectedActions, nextAction];

        return new ActionSelector(this.gameState, this.fen, newActions);
    }

    private _getNextStep(): ActionSelectorNextStep {
        let possibleNextActions = [];

        for (const outcome of this.allOutcomes) {
            if (!isListSubset(this.selectedActions, outcome.actions)) {
                continue;
            }

            if (this.selectedActions.length === outcome.actions.length) {
                return {
                    isDone: true,
                    value: outcome,
                }
            } else {
                const nextAction = outcome.actions[this.selectedActions.length];
                if (isListDeepContain(possibleNextActions, nextAction)) {
                    // Noop
                } else {
                    possibleNextActions.push(nextAction);
                }
            }
        }

        if (possibleNextActions.length === 0) {
            return {
                isDone: true,
                value: {
                    next_state: this.fen,
                    actions: [],
                }
            }
        } else {
            return {
                isDone: false,
                options: possibleNextActions,
            }
        }
    }

    tryConsumeInput(square: SquareType | null): ActionSelector | null {
        if (this.nextStep.isDone) {
            return this;
        }

        const interactions = this.nextStep.options;

        if (square !== null) {
            for (const interaction of interactions) {
                if (intersectionMatchType(square, interaction)) {
                    return this.getSelectorForNextAction(interaction);
                }
            }
        }

        // End turn is done by clicking anywhere not valid
        for (const interaction of interactions) {
            if (interaction.type === 'end_turn') {
                return this.getSelectorForNextAction(interaction);
            }
        }

        return null;
    }
}
