import './FullGamePlayer.css';
import { getWinner, isGameOver, Player, playerToPrettyColorString, type GameState, type PlayerType, type SquareType } from '../common/game_state';
import { describeAction, getPrettyGameStateFromFen, getPrettyGameStateWithActions, type PlayerAction } from '../common/api';
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
    const gameState = getPrettyGameStateFromFen(fen);

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
    const [gameStateHistories, setGameStateHistories] = useState<Array<string>>([initialFen]);
    const fenRef = useRef(state.fen);

    const {
        gameState,
        selector,
        completedActions,
        followingActions
    } = state;

    const isAiTurn = getIsAiTurn(gameState.acting_player, props.aiConfig);
    const isHumanVsHuman = !props.aiConfig?.p1Ai && !props.aiConfig?.p2Ai;

    let undoTurn: (() => void) | null = null;
    if (props.aiConfig?.p1Ai && props.aiConfig.p2Ai) {
        // Can never undo in AI vs AI
    } else if (isHumanVsHuman || isAiTurn) {
        if (gameStateHistories.length > 1) {
            undoTurn = () => {
                const prevGameState = gameStateHistories[gameStateHistories.length - 2];
                setGameStateHistories(gameStateHistories.slice(0, -1));
                setState(_getFreshState(prevGameState));
                setLastAiResponse(null);
            }
        }
    } else {
        // Human turn, need to undo the last ai turn
        if (gameStateHistories.length > 2) {
            undoTurn = () => {
                const prevGameState = gameStateHistories[gameStateHistories.length - 3];
                setGameStateHistories(gameStateHistories.slice(0, -2));
                setState(_getFreshState(prevGameState));
                setLastAiResponse(null);
            }
        }
    }

    function updateStateWithNewSelector(selector: ActionSelector) {
        let followingActions: Array<PlayerAction>;
        const nextStep = selector.nextStep;
        if (nextStep.isDone) {
            setState(_getFreshState(nextStep.value.next_state));
            setGameStateHistories([...gameStateHistories, nextStep.value.next_state]);
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
                setGameStateHistories([...gameStateHistories, result.next_state]);
            }
        }
    }

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

    const restartTurn = () => {
        setState(_getFreshState(state.fen));
    };

    const retry = () => {
        setGameStateHistories([initialFen]);
        setState(_getFreshState(initialFen));
        setLastAiResponse(null);
    };

    const endTurn = () => {
        onGridClicked(null);
    };

    const sidebarProps: GameSidebarProps = {
        state: state,
        aiConfig: props.aiConfig,
        restartTurn: restartTurn,
        endTurn: endTurn,
        undoTurn: undoTurn,
        retry: retry,
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

    const gameStateForRender = getPrettyGameStateWithActions(state.fen, completedActions);
    return (
        <div className='game-full-container'>
            <div className='game-grid-container'>
                <GameGridCanvas fen={state.fen} gameState={gameStateForRender} onClick={onGridClicked} availableActions={renderableActions} />
            </div>
            <div className='game-side-options'>
                {sidebar}
            </div>
        </div>
    );
}

function scoreToString(score: number) {
    if (score > 9000) {
        const dist = 10_000 - score;
        return `Win in ${dist}`;
    } else if (score < -9000) {
        const dist = 10_000 + score;
        return `Lose in ${dist}`;
    }

    return `${(sigmoid(score / 400) * 100).toFixed(2)}% to win`;
}

type CollapsibleSectionProps = {
    title: string,
    children: React.ReactNode,
};
function CollapsibleSection({ title, children }: CollapsibleSectionProps) {
    const [isOpen, setIsOpen] = useState(true);

    const toggleCollapse = () => {
        setIsOpen(!isOpen);
    };

    return (
        <div>
            <h3 onClick={toggleCollapse} aria-expanded={isOpen}>
                {title} {isOpen ? '▲' : '▼'}
            </h3>
            {isOpen && (
                <div style={{ transition: 'height 0.3s ease-in-out', overflow: 'hidden' }}>
                    {children}
                </div>
            )}
        </div>
    );
}


type GameSidebarProps = {
    state: FullGamePlayerState,
    aiConfig?: AiConfig,
    restartTurn: () => void,
    endTurn: () => void,
    undoTurn: (() => void) | null,
    returnToMenu: () => void,
    retry: () => void,
    lastAiResponse: SearchResult | null,
    isAiThinking: boolean,
}
function GameSidebar(props: GameSidebarProps) {
    const { state, lastAiResponse, returnToMenu, retry, isAiThinking } = props;
    const { gameState } = state;

    let toPlayText;
    const winner = getWinner(gameState);
    if (winner === null) {
        if (isAiThinking) {
            toPlayText = `${playerToPrettyColorString(state.gameState.acting_player)} is thinking...`;
        } else {
            toPlayText = `${playerToPrettyColorString(state.gameState.acting_player)} to play`;
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
                    <div>
                        <button onClick={retry} className='back-button'>
                            Retry Match
                        </button>
                        <button onClick={returnToMenu} className='back-button'>
                            Back to Menu
                        </button>
                    </div>
                }
            </div>
            <div className='game-sidebar-half'>
                {lastAiResponse === null ? null :
                    <CollapsibleSection title={"Last AI Turn"}>
                        <ul>
                            {lastAiResponse.meta.actions.map((action, idx) => (
                                <li key={idx}>{describeAction(action)}</li>
                            ))}

                            <br />

                            <li key='win'>
                                Prediction: {scoreToString(lastAiResponse.meta.score)}
                            </li>
                            <li key='nodes'>
                                Nodes checked: {lastAiResponse.meta.nodes_visited}
                            </li>
                            <li key='ply'>
                                Ply: {lastAiResponse.meta.calculated_depth}
                            </li>
                        </ul>
                    </CollapsibleSection>
                }

                <div className='game-sidebar-filler' />

                <button onClick={retry} className='back-button'>
                    Retry Match
                </button>
                <button onClick={returnToMenu} className='back-button'>
                    Back to Menu
                </button>
            </div>
        </div>

    )
}

function PlayerActionPanel({ state, restartTurn, undoTurn }: GameSidebarProps) {
    const completedActions = state.completedActions;

    return (
        <div style={{ width: '100%' }}>
            <button onClick={restartTurn} className='undo-button' disabled={completedActions.length === 0} >
                Redo Turn
            </button>

            <button onClick={() => undoTurn?.()} className='undo-button' disabled={undoTurn === null} >
                Undo Last Turn
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
