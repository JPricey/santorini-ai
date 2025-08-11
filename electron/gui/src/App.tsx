import { useState } from 'react';
import './App.css';
import { GameGrid } from './GameGrid';
import { GodMultiselect } from './GodSelector';
import { MatchupsTable } from './MatchupsTable';
import { Engine } from './Engine';

function App() {
    const [selectedGods, setSelectedGods] = useState<string[]>([]);

    return (
        <div className="App">
            <Engine />
            <GodMultiselect onSelectedChanged={setSelectedGods} />
            <MatchupsTable selectedGods={selectedGods} />
        </div>
    );
}

export default App;
