import { getNextMoves, PlayerActionTypes, type NextState, type PlayerAction, type PlayerActionType } from "./api";
import { isGameOver, squareToSquareStr, type GameState, type SquareType } from "./game_state";
import { assertUnreachable, isListDeepContain, isListSubset } from "./utils";

export type ActionSelectorNextStep = {
    isDone: true,
    value: NextState,
} | {
    isDone: false,
    options: Array<PlayerAction>,
};

export function intersectionMatchType(square: SquareType, action: PlayerAction): PlayerActionType | null {
    const squareStr = squareToSquareStr(square);

    switch (action.type) {
        case PlayerActionTypes.PlaceWorker:
        case PlayerActionTypes.SelectWorker:
        case PlayerActionTypes.MoveWorker:
        case PlayerActionTypes.Build:
        case PlayerActionTypes.Dome:
            return action.value === squareStr ? action.type : null;
        case PlayerActionTypes.MoveWorkerWithPush:
        case PlayerActionTypes.MoveWorkerWithSwap:
            return action.value[0] === squareStr ? action.type : null;
        case PlayerActionTypes.EndTurn:
        case PlayerActionTypes.NoMoves:
            return null;
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
                for (const action of nextState.actions) {
                    if (action.type === PlayerActionTypes.MoveWorkerWithPush) {
                        console.log(action);
                        console.log(JSON.stringify(action));
                    }
                }
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
