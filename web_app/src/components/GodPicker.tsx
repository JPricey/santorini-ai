import { God, type GodType } from "../common/game_state";
import { capitalizeFirstLetter } from "../common/utils";
import './GodPicker.css'

type GodPickerProps = {
    value: GodType;
    onChange: (god: GodType) => void;
    isHuman: boolean;
    onToggleHuman: (isHuman: boolean) => void;
};

export function GodPicker({ value, onChange, isHuman, onToggleHuman }: GodPickerProps) {
    return (
        <div className="GodPickerList">
            {Object.values(God).map(god => (
                <button
                    key={god}
                    onClick={() => onChange(god)}
                    className={`GodPickerButton${god === value ? " Selected" : ""}`}
                >
                    {capitalizeFirstLetter(god)}
                </button>
            ))}

            <div style={{ marginBottom: 16, display: 'flex', alignItems: 'center' }}>
                <span style={{ marginRight: 12, fontWeight: isHuman ? 'bold' : 'normal' }}>Human</span>
                <label style={{ position: 'relative', display: 'inline-block', width: 48, height: 24, margin: '0 8px' }}>
                    <input
                        type="checkbox"
                        checked={!isHuman ? true : false}
                        onChange={() => onToggleHuman(!isHuman)}
                        style={{
                            opacity: 0,
                            width: 0,
                            height: 0,
                        }}
                    />
                    <span
                        style={{
                            position: 'absolute',
                            cursor: 'pointer',
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            backgroundColor: isHuman ? '#ccc' : '#2196F3',
                            borderRadius: 24,
                            transition: '0.2s',
                        }}
                    />
                    <span
                        style={{
                            position: 'absolute',
                            left: isHuman ? 2 : 26,
                            top: 2,
                            width: 20,
                            height: 20,
                            backgroundColor: '#fff',
                            borderRadius: '50%',
                            transition: '0.2s',
                            boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
                        }}
                    />
                </label>
                <span style={{ marginLeft: 12, fontWeight: !isHuman ? 'bold' : 'normal' }}>AI</span>
            </div>
        </div>
    );
}
