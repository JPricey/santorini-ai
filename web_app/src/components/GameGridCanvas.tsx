import { useEffect, useRef, useState } from 'react';
import { getPlayerOnSquare, playerToString, Player, type GameState, type SquareType, type PlayerType, getTokenOnSquare } from '../common/game_state';
import './GameGridCanvas.css';
import { describeActionType, PlayerActionTypes, type PlayerAction, type PlayerActionType } from '../common/api';
import { intersectionMatchType } from '../common/action_selector';
import { assertUnreachable } from '../common/utils';

export type GameGridCanvasProps = {
    gameState: GameState;
    fen: string,
    availableActions?: Array<PlayerAction>,
    onClick?: (square: SquareType | null) => void;
};

export function GameGridCanvas({ gameState, fen, onClick, availableActions }: GameGridCanvasProps) {
    const containerRef = useRef<HTMLDivElement | null>(null);
    const [size, setSize] = useState<{ width: number; height: number }>({
        width: 0,
        height: 0,
    });

    useEffect(() => {
        const container = containerRef.current;
        if (!container) return;

        const resizeObserver = new window.ResizeObserver(entries => {
            for (let entry of entries) {
                if (entry && entry.contentRect) {
                    setSize({
                        width: Math.floor(entry.contentRect.width),
                        height: Math.floor(entry.contentRect.height),
                    });
                }
            }
        });
        resizeObserver.observe(container);

        setSize({
            width: Math.floor(container.offsetWidth),
            height: Math.floor(container.offsetHeight),
        });

        return () => {
            resizeObserver.disconnect();
        };
    }, [containerRef]);


    const innerWidth = Math.min(size.width, size.height);
    const svgBoardProps = {
        fen: fen,
        gameState: gameState,
        width: innerWidth,
        height: innerWidth,
        onClick: onClick,
        availableActions: availableActions,
    };
    return (
        <div ref={containerRef} className='outer-div'>
            <div onClick={() => onClick?.(null)} className='inner-div' style={{ maxWidth: innerWidth, maxHeight: innerWidth }}>
                <SvgBoard {...svgBoardProps} />
            </div>
        </div>
    );
}

type SvgBoardProps = {
    gameState: GameState;
    fen: string;
    width: number;
    height: number;
    onClick?: (square: SquareType | null) => void;
    availableActions?: Array<PlayerAction>,
};

function sinDeg(rad: number) {
    return Math.sin(rad * (Math.PI / 180));
}

function cosDeg(rad: number) {
    return Math.cos(rad * (Math.PI / 180));
}

// All sizes are relative and scaled to STANDARD_WIDTH
const STANDARD_WIDTH = 1024.0;
const EDGE_BUFFER_PROPORTION = 0.9;
const GAME_BOARD_RENDER_MAX_WIDTH = STANDARD_WIDTH * EDGE_BUFFER_PROPORTION;
const SINGLE_SIDE_EDGE_BUFFER = STANDARD_WIDTH * (1 - EDGE_BUFFER_PROPORTION) / 2;


// Angle through the axis looking straight down at the board
const ISO_ROTATE_ANGLE = 10.0;
// Angle giving the 3D appearance of the board
const ISO_TILT_ANGLE = 30.0;
const LAYER_HEIGHT = 30.0;

const SIN_ROTATE = sinDeg(ISO_ROTATE_ANGLE);
const COS_ROTATE = cosDeg(ISO_ROTATE_ANGLE);

const SIN_TILT = sinDeg(ISO_TILT_ANGLE);
const COS_TILT = cosDeg(ISO_TILT_ANGLE);

const GAME_BOARD_REAL_WIDTH = GAME_BOARD_RENDER_MAX_WIDTH / (sinDeg(ISO_ROTATE_ANGLE) + cosDeg(ISO_ROTATE_ANGLE));
const SQUARE_REAL_WIDTH = GAME_BOARD_REAL_WIDTH / 5;
const HEIGHT_WIDTH_PROPORTIONS = [
    0.975,
    0.9,
    0.75,
    0.6,
    0.5,
];

const X_ORIG = SINGLE_SIDE_EDGE_BUFFER * 1.3;
const Y_ORIG = SINGLE_SIDE_EDGE_BUFFER * 2.0 + GAME_BOARD_REAL_WIDTH * sinDeg(ISO_ROTATE_ANGLE);

function transformPoint(x: number, y: number) {
    const nx = x * COS_ROTATE + y * SIN_ROTATE;
    const ny = (y * COS_ROTATE - x * SIN_ROTATE) * COS_TILT;
    return [nx, ny];
}

function polyCornersForPercentSize(pct: number) {
    const low = (1 - pct) / 2;
    const high = 1 - low;

    return [
        transformPoint(low, low),
        transformPoint(high, low),
        transformPoint(high, high),
        transformPoint(low, high),
    ]
}

const SQUARE_RENDERED_BOUND_X = SQUARE_REAL_WIDTH * (SIN_ROTATE + COS_ROTATE);
const SQUARE_RENDERED_BOUND_Y = SQUARE_REAL_WIDTH * (COS_ROTATE - SIN_ROTATE) * COS_TILT;

function actionToFill(actionType: PlayerActionType): string {
    switch (actionType) {
        case PlayerActionTypes.PlaceWorker:
            return 'yellow'
        case PlayerActionTypes.SelectWorker:
            return 'blue'
        case PlayerActionTypes.MoveWorker:
            return 'green'
        case PlayerActionTypes.Build:
            return 'red'
        case PlayerActionTypes.Dome:
        case PlayerActionTypes.SetTalusPosition:
            return 'purple'
        case PlayerActionTypes.EndTurn:
            return 'white'
        case PlayerActionTypes.NoMoves:
            return 'black'
        case PlayerActionTypes.SetWindDirection:
            return 'cyan'
        default:
            return assertUnreachable(actionType);
    }
}

const LEGEND_TEXT_SIZE = 20;
const LEGEND_LINE_SPACING = 22;
const LEGEND_BOX_SIZE = 18;
const LEGEND_BOX_DELTA = -LEGEND_TEXT_SIZE * 0.85;

const EXTRA_INFO_TEXT_SIZE = 16;

function SvgBoard({ gameState, width, height, onClick, availableActions }: SvgBoardProps) {
    if (width === 0 || height === 0) {
        return null;
    }

    function c(x: number) {
        const ratio = width / STANDARD_WIDTH;
        return x * ratio;
    }


    function isoRoot(row: number, col: number) {
        const x = X_ORIG + SQUARE_REAL_WIDTH * (col * COS_ROTATE + row * SIN_ROTATE);
        const y = Y_ORIG + SQUARE_REAL_WIDTH * (row * COS_ROTATE - col * SIN_ROTATE) * COS_TILT;

        return [x, y];
    }

    const polyString = polyCornersForPercentSize(0.975).map((p) => `${c(p[0] * SQUARE_REAL_WIDTH)},${c(p[1] * SQUARE_REAL_WIDTH)}`).join(' ');

    const underPoly = isoRoot(2, 2);
    const underPolyString = polyCornersForPercentSize(5).map((p) => `${c(p[0] * SQUARE_REAL_WIDTH)},${c(p[1] * SQUARE_REAL_WIDTH)}`).join(' ');

    function getActionMatch(square: SquareType): PlayerActionType | null {
        for (const action of availableActions ?? []) {
            const matchResult = intersectionMatchType(square, action);
            if (matchResult) {
                return matchResult;
            }
        }

        for (const action of availableActions ?? []) {
            if (action.type == PlayerActionTypes.EndTurn) {
                return action.type;
            }
        }

        return null;
    }

    let validActionTypes: Array<PlayerActionType> = [];
    for (const action of availableActions ?? []) {
        if (!validActionTypes.includes(action.type)) {
            validActionTypes.push(action.type);
        }
    }

    function specialPlayerTextFull(player: PlayerType, innerText: string | null): string {
        if (innerText) {
            return `Player ${playerToString(player)}: ${innerText}`;
        }

        return '';
    }

    const p1SpecialText = specialPlayerTextFull(Player.One, gameState.players[0].special_text);
    const p2SpecialText = specialPlayerTextFull(Player.Two, gameState.players[1].special_text);

    return (
        <svg style={{ height: '100%', width: '100%' }}>
            <defs>
                <radialGradient id="player1Gradient" cx="30%" cy="20%" r="80%">
                    <stop offset="0%" stopColor="#EEE" />
                    <stop offset="80%" stopColor="#CCC" />
                    <stop offset="100%" stopColor="#AAA" />
                </radialGradient>
                <radialGradient id="player2Gradient" cx="30%" cy="20%" r="80%">
                    <stop offset="0%" stopColor="#888" />
                    <stop offset="80%" stopColor="#222" />
                    <stop offset="100%" stopColor="#111" />
                </radialGradient>
            </defs>

            <g key='underside' className="no-hover underside" transform={`translate(${c(underPoly[0])},${c(underPoly[1])})`}>
                <polygon points={underPolyString} />
            </g>

            {p1SpecialText === "" ? null :
                <g key='p1SpecialText' transform={`translate(${c(30)},${c(30)})`}>
                    <text fontSize={c(EXTRA_INFO_TEXT_SIZE)}>
                        {p1SpecialText}
                    </text>
                </g>
            }

            {p2SpecialText === "" ? null :
                <g key='p2SpecialText' transform={`translate(${c(600)},${c(30)})`}>
                    <text fontSize={c(EXTRA_INFO_TEXT_SIZE)}>
                        {p2SpecialText}
                    </text>
                </g>
            }

            {validActionTypes.length === 0 ? null :
                <g key='legend' transform={`translate(${c(30)},${c(120)})`}>
                    <text fontSize={c(LEGEND_TEXT_SIZE)} fontWeight='bold'>
                        Take an action:
                    </text>
                    {validActionTypes.map((actionType, idx) => (
                        <g key={actionType} transform={`translate(0, ${c(idx + 1) * LEGEND_LINE_SPACING})`}>
                            <rect
                                x={c(5)}
                                y={c(LEGEND_BOX_DELTA)}
                                width={c(LEGEND_BOX_SIZE)}
                                height={c(LEGEND_BOX_SIZE)}
                                fill={actionToFill(actionType)}
                                stroke="#222"
                                strokeWidth={c(1)}
                                rx={c(2)}
                            />

                            <text x={c(LEGEND_BOX_SIZE + 10)} fontSize={c(LEGEND_TEXT_SIZE)}>
                                {describeActionType(actionType)}
                            </text>
                        </g>
                    ))}
                </g>
            }

            {
                new Array(25).fill(0).map((_, i) => {
                    const row = Math.floor(i / 5);
                    const col = 4 - i % 5;
                    i = row * 5 + col;
                    const square = i as SquareType;
                    const squareHeight = gameState.heights[row][col];
                    const [x, y] = isoRoot(row, col);
                    const squareMidX = c(SQUARE_RENDERED_BOUND_X / 2);
                    const squareMidY = c(SQUARE_REAL_WIDTH / 2 * (COS_ROTATE - SIN_ROTATE) * COS_TILT);
                    const cyHeightDelta = c(SIN_TILT * LAYER_HEIGHT * squareHeight);
                    const topMidY = squareMidY - cyHeightDelta;

                    const player = getPlayerOnSquare(gameState, square);
                    const token = getTokenOnSquare(gameState, square);
                    const action = getActionMatch(square);
                    let actionPathSizePct = 0.9;
                    if (squareHeight > 0) {
                        actionPathSizePct *= HEIGHT_WIDTH_PROPORTIONS[squareHeight - 1];
                    }
                    const actionPath = polyCornersForPercentSize(actionPathSizePct).map((p) => `${c(p[0] * SQUARE_REAL_WIDTH)},${c(p[1] * SQUARE_REAL_WIDTH) - cyHeightDelta}`).join(' ');

                    function workerEllipse() {
                        if (player === null) {
                            return null;
                        }
                        const ovalRadiusX = c(SQUARE_RENDERED_BOUND_X * 0.15);
                        const ovalRadiusY = c(SQUARE_RENDERED_BOUND_Y * 0.5);
                        const ovalCenterX = squareMidX;
                        const ovalCenterY = topMidY - ovalRadiusY * 0.4;
                        return (
                            <ellipse
                                cx={ovalCenterX}
                                cy={ovalCenterY}
                                rx={ovalRadiusX}
                                ry={ovalRadiusY}
                                fill={player === Player.One ? "url(#player1Gradient)" : "url(#player2Gradient)"}
                                strokeWidth={c(3)}
                            />
                        );
                    }

                    function tokenTriangle() {
                        if (token === null) {
                            return null;
                        }
                        const ovalRadiusX = c(SQUARE_RENDERED_BOUND_X * 0.2);
                        const ovalRadiusY = c(SQUARE_RENDERED_BOUND_Y * 0.15);
                        const ovalCenterX = squareMidX;
                        const ovalCenterY = topMidY - ovalRadiusY * 0.4;
                        return (
                            <ellipse
                                cx={ovalCenterX}
                                cy={ovalCenterY}
                                rx={ovalRadiusX}
                                ry={ovalRadiusY}
                                fill={token === Player.One ? "url(#player1Gradient)" : "url(#player2Gradient)"}
                                strokeWidth={c(3)}
                            />
                        );
                    }

                    return (
                        <g key={`${row}-${col}`} transform={`translate(${c(x)},${c(y)})`} onClick={(e) => {
                            e.stopPropagation();
                            onClick?.(square)
                        }}>
                            <polygon points={polyString} className='ground' strokeWidth={c(1)} />
                            {
                                Array(squareHeight).fill(0).map((_, j) => {
                                    const topYDelta = -(j + 1) * LAYER_HEIGHT * SIN_TILT;
                                    const polyString = polyCornersForPercentSize(HEIGHT_WIDTH_PROPORTIONS[j]).map((p) => `${c(p[0] * SQUARE_REAL_WIDTH)},${c(p[1] * SQUARE_REAL_WIDTH + topYDelta)}`).join(' ');

                                    const isDome = j === 3;
                                    return (
                                        <polygon key={j} points={polyString} className={isDome ? ' dome' : 'building'} strokeWidth={c(3)} />
                                    );
                                })
                            }
                            {squareHeight === 4 ? null :
                                <text x={squareMidX} y={topMidY} textAnchor="middle" dominantBaseline="central" fontSize={c(75)}

                                    transform={`scale(1.0 ${COS_TILT}) rotate(-${ISO_ROTATE_ANGLE}, ${squareMidX}, ${topMidY}) `}
                                    className="height-text"
                                >
                                    {squareHeight}
                                </text>
                            }
                            {
                                action === null ? null :
                                    <polygon key='action' points={actionPath} fill={actionToFill(action)} className='action-selection' />
                            }
                            {tokenTriangle()}
                            {workerEllipse()}
                        </g>
                    );
                })
            }

            <g>
                {Array.from({ length: 5 }).map((_, col) => {
                    const [x, y] = isoRoot(4.7, col);
                    const squareMidX = c(x + SQUARE_RENDERED_BOUND_X / 2);
                    const squareMidY = c(y + SQUARE_REAL_WIDTH / 2 * (COS_ROTATE - SIN_ROTATE) * COS_TILT);
                    return (
                        <text
                            key={`footer-${col}`}
                            x={squareMidX}
                            y={squareMidY}
                            textAnchor="middle"
                            dominantBaseline="hanging"
                            fontSize={c(60)}
                            className="height-text"
                        >
                            {String.fromCharCode(65 + col)}
                        </text>
                    );
                })}
            </g>

            <g>
                {Array.from({ length: 5 }).map((_, row) => {
                    const [x, y] = isoRoot(row, -0.8);
                    const squareMidX = c(x + SQUARE_RENDERED_BOUND_X * 0.5);
                    const squareMidY = c(y + SQUARE_REAL_WIDTH / 2 * (COS_ROTATE - SIN_ROTATE) * COS_TILT);
                    return (
                        <text
                            key={`side-${row}`}
                            x={squareMidX}
                            y={squareMidY}
                            textAnchor="middle"
                            dominantBaseline="middle"
                            fontSize={c(60)}
                            className="height-text"
                        >
                            {5 - row}
                        </text>
                    );
                })}
            </g>
        </svg >

    );
}

