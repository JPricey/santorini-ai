import { useState } from "react";
import { GodPicker } from "./GodPicker";
import { God, type GodType } from "../common/game_state";
import { FullGamePlayer, type AiConfig } from "./FullGamePlayer";
import type { AiWorker } from "../ai/ai_worker";
import { assertUnreachable } from "../common/utils";
import './MenuScreen.css';

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
            return 100;
        case AiSpeed.Medium:
            return 2 * 1000;
        case AiSpeed.Long:
            return 10 * 1000;
        default:
            return assertUnreachable(aiSpeed);
    }
}

type AiSpeedPickerProps = {
    aiSpeed: AiSpeedType,
    setAiSpeed: (aiSpeed: AiSpeedType) => void;
};
function AiSpeedPicker({ aiSpeed, setAiSpeed }: AiSpeedPickerProps) {
    return (
        <div className="ai-speed-picker">
            <label style={{ marginRight: 16 }}>AI Thinking Time:</label>
            <label style={{ marginRight: 12 }} className="option">
                <input type="radio" name="ai-time" checked={aiSpeed === AiSpeed.Short} onChange={() => setAiSpeed(AiSpeed.Short)} />
                Short
            </label>
            <label style={{ marginRight: 1 }} className="option">
                <input type="radio" name="ai-time" checked={aiSpeed === AiSpeed.Medium} onChange={() => setAiSpeed(AiSpeed.Medium)} />
                Medium
            </label>
            <label className="option">
                <input type="radio" name="ai-time" checked={aiSpeed === AiSpeed.Long} onChange={() => setAiSpeed(AiSpeed.Long)} />
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
                    <div className="menu-screen-player-options">
                        <div className="menu-player-section">
                            <h3>Player One</h3>
                            <GodPicker value={p1God} onChange={setP1God} isHuman={isP1Human} onToggleHuman={setP1Human} />
                        </div>
                        <div className="menu-player-section">
                            <h3>Player Two</h3>
                            <GodPicker value={p2God} onChange={setP2God} isHuman={isP2Human} onToggleHuman={setP2Human} />
                        </div>
                    </div>

                    <AiSpeedPicker aiSpeed={aiSpeed} setAiSpeed={setAiSpeed} />

                    <button
                        style={{
                            padding: "0.8rem 2rem",
                            fontSize: "1.2rem",
                            background: "#2196F3",
                            color: "#fff",
                            border: "none",
                            borderRadius: 8,
                            cursor: "pointer",
                            fontWeight: 600,
                            boxShadow: "0 2px 8px rgba(0,0,0,0.08)"
                        }}
                        onClick={startGame}
                    >
                        Start Game
                    </button>
                </div>
            </div>
        );
    }
}
