import { getAllGodNames } from "./common/gods";
import Select from 'react-select'
import { useState } from "react";

export function GodMultiselect({ onSelectedChanged }: { onSelectedChanged: (selectedGods: string[]) => void }) {
    const allGods = getAllGodNames();
    const options: any = allGods.map((god) => {
        return { value: god, label: god }
    });

    const [multiValue, setMultiValue] = useState([]);

    const onChange = (selectedOptions: any) => {
        setMultiValue(selectedOptions)
        const namesOnly = selectedOptions.map((option: any) => option.value);
        onSelectedChanged(namesOnly);
    }

    return (
        <div>
            <Select
                isMulti
                options={options}
                onChange={onChange}
                value={multiValue}
                placeholder="Select Gods..."
            />
        </div>
    );
}
