import { useState, useMemo } from "react";
import { GodPicker } from "./GodPicker";
import { God, type GodType, WIP_GODS } from "../common/game_state";
import { FullGamePlayer, type AiConfig } from "./FullGamePlayer";
import type { AiWorker } from "../ai/ai_worker";
import { assertUnreachable } from "../common/utils";
import './MenuScreen.css';
import { getBannedMatchups } from '../common/api'

export type MenuScreenProps = {
    aiWorker?: AiWorker,
};

export const AiSpeed = {
    Short: 'short',
    Medium: 'medium',
    Long: 'long',
} as const;
export type AiSpeedType = typeof AiSpeed[keyof typeof AiSpeed];

export function getAiSpeedDuration(aiSpeed: AiSpeedType): number {
    switch (aiSpeed) {
        case AiSpeed.Short:
            return 50;
        case AiSpeed.Medium:
            return 1 * 1000;
        case AiSpeed.Long:
            return 5 * 1000;
        default:
            return assertUnreachable(aiSpeed);
    }
}

const SECRET_CLICK_ORDER = [
    AiSpeed.Long,
    AiSpeed.Medium,
    AiSpeed.Long,
    AiSpeed.Medium,
    AiSpeed.Short,
] as const;
type AiSpeedPickerProps = {
    aiSpeed: AiSpeedType,
    setAiSpeed: (aiSpeed: AiSpeedType) => void;
    setSecretCompleted?: () => void;
};
function AiSpeedPicker({ aiSpeed, setAiSpeed, setSecretCompleted }: AiSpeedPickerProps) {
    const [clickChainIdx, setClickChainIdx] = useState(0);

    function onPressed(aiSpeed: AiSpeedType) {
        if (clickChainIdx < SECRET_CLICK_ORDER.length) {
            if (aiSpeed === SECRET_CLICK_ORDER[clickChainIdx]) {
                setClickChainIdx(clickChainIdx + 1);
                if (clickChainIdx + 1 >= SECRET_CLICK_ORDER.length) {
                    setSecretCompleted?.();
                }
            } else {
                setClickChainIdx(0);
            }

        }
        setAiSpeed(aiSpeed);
    }

    return (
        <div className="ai-speed-picker">
            <label style={{ marginRight: 16 }}>AI Thinking Time:</label>
            <label style={{ marginRight: 12 }} className="option">
                <input type="radio" name="ai-time" checked={aiSpeed === AiSpeed.Short} onChange={() => onPressed(AiSpeed.Short)} />
                Short
            </label>
            <label style={{ marginRight: 1 }} className="option">
                <input type="radio" name="ai-time" checked={aiSpeed === AiSpeed.Medium} onChange={() => onPressed(AiSpeed.Medium)} />
                Medium
            </label>
            <label className="option">
                <input type="radio" name="ai-time" checked={aiSpeed === AiSpeed.Long} onChange={() => onPressed(AiSpeed.Long)} />
                Long
            </label>
        </div>
    );
}

export function MenuScreen(props: MenuScreenProps) {
    const [p1God, setP1God] = useState<GodType>(God.Mortal);
    const [p2God, setP2God] = useState<GodType>(God.Mortal);
    const [isP1Human, setP1Human] = useState<boolean>(true);
    const [isP2Human, setP2Human] = useState<boolean>(false);
    const [aiSpeed, setAiSpeed] = useState<AiSpeedType>(AiSpeed.Short);
    const [isGameRunning, setGameIsRunning] = useState<boolean>(false);
    const [fen, setFen] = useState<string>('');
    const [aiConfig, setAiConfig] = useState<AiConfig | null>(null);
    const [isSecretCompleted, setSecretCompleted] = useState(true);

    const godOptions = useMemo(() => {
        if (isSecretCompleted) {
            return Object.values(God);
        } else {
            return Object.values(God).filter((god) => !WIP_GODS.has(god));
        }
    }, [isSecretCompleted]);

    const bannedMatches = useMemo(() => getBannedMatchups(), []);
    const isMatchupBanned = useMemo(() => {
        const matchupStr = `${p1God}|${p2God}`;
        return bannedMatches.has(matchupStr);
    }, [p1God, p2God, bannedMatches]);

    const startGame = () => {
        if (isGameRunning) {
            return;
        }
        const fen = `00000 00000 00000 00000 00000/1/${p1God}/${p2God}`;
        if (props.aiWorker) {
            props.aiWorker.clearMap();
            setAiConfig({
                aiWorker: props.aiWorker,
                p1Ai: !isP1Human,
                p2Ai: !isP2Human,
                aiSpeed: aiSpeed,
            })
        }
        setFen(fen);
        setGameIsRunning(true);
    };


    if (isGameRunning) {
        return (
            <FullGamePlayer fen={fen} aiConfig={aiConfig ?? undefined} gameIsDoneCallback={() => setGameIsRunning(false)} />
        );
    } else {
        return (
            <div className="menu-screen-container">
                <div className="menu-screen-content">
                    <div style={{ height: '10px' }} />
                    <button
                        className="menu-start-btn"
                        onClick={startGame}
                        disabled={isMatchupBanned}
                    >
                        {
                            isMatchupBanned ? "Banned Matchup" : "Start Game"
                        }
                    </button>

                    <AiSpeedPicker aiSpeed={aiSpeed} setAiSpeed={setAiSpeed} setSecretCompleted={() => setSecretCompleted(true)} />

                    <div className="menu-screen-player-options">
                        <div className="menu-player-section">
                            <h3>Player One</h3>
                            <GodPicker value={p1God} options={godOptions} onChange={setP1God} isHuman={isP1Human} onToggleHuman={setP1Human} />
                        </div>
                        <div className="menu-player-section">
                            <h3>Player Two</h3>
                            <GodPicker value={p2God} options={godOptions} onChange={setP2God} isHuman={isP2Human} onToggleHuman={setP2Human} />
                        </div>
                    </div>

                </div>
            </div>
        );
    }
}
