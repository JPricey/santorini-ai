import './FullGamePlayer.css';
import { parseFen } from "../common/fen";
import { getWinner, isGameOver, Player, playerToPrettyColorString, type GameState, type PlayerType, type SquareType } from '../common/game_state';
import { describeAction, gameStateWithActions, type PlayerAction } from '../common/api';
import { ActionSelector } from '../common/action_selector';
import { useEffect, useRef, useState } from 'react';
import type { AiWorker, SearchResult } from '../ai/ai_worker';
import { GameGridCanvas } from './GameGridCanvas';
import { assertUnreachable, capitalizeFirstLetter, setTimeoutAsync, sigmoid } from '../common/utils';
import { getAiSpeedDuration, type AiSpeedType } from './MenuScreen';

export type AiConfig = {
    aiWorker: AiWorker,
    aiSpeed: AiSpeedType,
    p1Ai: boolean,
    p2Ai: boolean,
}

export type FullGamePlayerProps = {
    fen?: string,
    aiConfig?: AiConfig,
    gameIsDoneCallback: () => void,
};

type FullGamePlayerState = {
    fen: string,
    gameState: GameState,
    selector: ActionSelector,
    completedActions: Array<PlayerAction>,
    followingActions: Array<PlayerAction>,
}

function getIsAiTurn(player: PlayerType, aiConfig: AiConfig | undefined): boolean {
    if (!aiConfig) {
        return false;
    }

    switch (player) {
        case Player.One:
            return aiConfig.p1Ai;
        case Player.Two:
            return aiConfig.p2Ai;
        default:
            return assertUnreachable(player);
    }
}

function _getFreshState(fen: string): FullGamePlayerState {
    const gameState = parseFen(fen).unwrap();
    const selector = new ActionSelector(gameState, fen, []);
    let followingActions: Array<PlayerAction>;
    const nextStep = selector.nextStep;
    if (nextStep.isDone) {
        followingActions = [];
    } else {
        followingActions = nextStep.options;
    }

    return {
        fen: fen,
        gameState: gameState,
        selector: selector,
        completedActions: [],
        followingActions: followingActions,
    }
}

export function FullGamePlayer(props: FullGamePlayerProps) {
    const initialFen = props.fen ?? '0000000000000000000000000/1/hermes:B3,D3/minotaur:C2,C4';
    const [state, setState] = useState(() => _getFreshState(initialFen));
    const [lastAiResponse, setLastAiResponse] = useState<SearchResult | null>(null);
    const [isAiThinking, setIsAiThinking] = useState<boolean>(false);
    const fenRef = useRef(state.fen);

    const {
        gameState,
        selector,
        completedActions,
        followingActions
    } = state;

    const isAiTurn = getIsAiTurn(gameState.currentPlayer, props.aiConfig);

    function updateStateWithNewSelector(selector: ActionSelector) {
        let followingActions: Array<PlayerAction>;
        const nextStep = selector.nextStep;
        if (nextStep.isDone) {
            setState(_getFreshState(nextStep.value.next_state));
            return;
        } else {
            followingActions = nextStep.options;
        }

        setState({
            fen: state.fen,
            gameState: state.gameState,
            selector: selector,
            completedActions: selector.selectedActions,
            followingActions: followingActions,
        });
    }

    async function maybeTriggerAi() {
        const startFen = fenRef.current;
        if (isGameOver(gameState)) {
            return;
        }
        if (!isAiTurn) {
            return;
        }
        if (props.aiConfig) {
            const { aiWorker, aiSpeed } = props.aiConfig;
            const aiDuration = getAiSpeedDuration(aiSpeed);
            setIsAiThinking(true);
            const now = Date.now();
            const result = await aiWorker.getAiResult(state.fen, aiDuration);
            const elapsedMs = Date.now() - now;
            if (elapsedMs < 100) {
                await setTimeoutAsync(100 - elapsedMs);
            }
            setIsAiThinking(false);

            const endFen = fenRef.current;

            if (startFen === endFen) {
                setLastAiResponse(result);
                setState(_getFreshState(result.next_state));
            }
        }
    }

    // Automatically advance when there's only 1 choice
    // useEffect(() => {
    //     if (followingActions.length === 1) {
    //         updateStateWithNewSelector(selector.getSelectorForNextAction(followingActions[0]));
    //     }
    // }, [state]);

    useEffect(() => {
        fenRef.current = state.fen;
        maybeTriggerAi();
    }, [state.fen]);

    const onGridClicked = (square: SquareType | null) => {
        if (isGameOver(state.gameState)) {
            props.gameIsDoneCallback()
            return;
        }

        if (isAiTurn) {
            return;
        }

        const nextSelector = selector.tryConsumeInput(square);
        if (nextSelector) {
            updateStateWithNewSelector(nextSelector);
        }
    };

    const undoTurn = () => {
        setState(_getFreshState(state.fen));
    };

    const endTurn = () => {
        onGridClicked(null);
    };

    const sidebarProps: GameSidebarProps = {
        state: state,
        aiConfig: props.aiConfig,
        undoTurn: undoTurn,
        endTurn: endTurn,
        lastAiResponse: lastAiResponse,
        returnToMenu: props.gameIsDoneCallback,
        isAiThinking: isAiThinking,
    };

    const sidebar = <GameSidebar {...sidebarProps} />;

    let renderableActions;
    if (isAiTurn) {
        renderableActions = undefined;
    } else {
        renderableActions = followingActions;
    }

    const gameStateForRender = gameStateWithActions(gameState, completedActions);

    return (
        <div className='game-full-container'>
            <div className='game-grid-container'>
                <GameGridCanvas gameState={gameStateForRender} onClick={onGridClicked} availableActions={renderableActions} />
            </div>
            <div className='game-side-options'>
                {sidebar}
            </div>
        </div>
    );
}

type GameSidebarProps = {
    state: FullGamePlayerState,
    aiConfig?: AiConfig,
    undoTurn: () => void,
    endTurn: () => void,
    returnToMenu: () => void,
    lastAiResponse: SearchResult | null,
    isAiThinking: boolean,
}
function GameSidebar(props: GameSidebarProps) {
    const { state, lastAiResponse, returnToMenu, isAiThinking } = props;
    const { gameState } = state;

    let toPlayText;
    const winner = getWinner(gameState);
    if (winner === null) {
        if (isAiThinking) {
            toPlayText = `${playerToPrettyColorString(state.gameState.currentPlayer)} is thinking...`;
        } else {
            toPlayText = `${playerToPrettyColorString(state.gameState.currentPlayer)} to play`;
        }
    } else {
        toPlayText = `${playerToPrettyColorString(winner)} wins!`;
    };

    const versusText = `${capitalizeFirstLetter(gameState.players[0].god)} vs ${capitalizeFirstLetter(gameState.players[1].god)}`;

    return (
        <div className="game-sidebar-container" >
            <div className='game-sidebar-half'>
                <h1>
                    {versusText}
                </h1>
                <h2>
                    {toPlayText}
                </h2>

                {winner === null ?
                    <PlayerActionPanel {...props} />
                    :
                    <button onClick={returnToMenu} className='back-button'>
                        Back to Menu
                    </button>
                }
            </div>
            <div className='game-sidebar-half'>
                {lastAiResponse === null ? null :
                    <div>
                        <h3>
                            Last AI turn:
                        </h3>

                        <ul>
                            {lastAiResponse.meta.actions.map((action, idx) => (
                                <li key={idx}>{describeAction(action)}</li>
                            ))}

                            <br />

                            <li key='win'>
                                Predicted Win Chance: {(sigmoid(lastAiResponse.meta.score / 400) * 100).toFixed(2)}%
                            </li>
                            <li key='nodes'>
                                Nodes checked: {lastAiResponse.meta.nodes_visited}
                            </li>
                            <li key='ply'>
                                Ply: {lastAiResponse.meta.calculated_depth}
                            </li>
                        </ul>
                    </div>
                }

                <div className='game-sidebar-filler' />

                <button onClick={returnToMenu} className='back-button'>
                    Back to Menu
                </button>
            </div>
        </div>

    )
}

function PlayerActionPanel({ state, undoTurn }: GameSidebarProps) {
    const completedActions = state.completedActions;

    return (
        <div style={{ width: '100%' }}>
            <button onClick={undoTurn} className='undo-button' disabled={completedActions.length === 0} >
                Redo Turn
            </button>

            <h3>
                Actions taken:
            </h3>

            <ul>
                {completedActions.map((action, idx) => (
                    <li key={idx}>{describeAction(action)}</li>
                ))}
            </ul>
        </div>
    )
}
