import init from "../pkg/wasm_app";
import { useState } from 'react';
import { AiWorker } from './ai/ai_worker';
import { MenuScreen } from './components/MenuScreen';

function App() {
    const [isLoaded, setLoaded] = useState(false);
    const [worker, _] = useState(() => {
        return new AiWorker();
    });

    if (isLoaded) {
        return <MenuScreen aiWorker={worker} />
    } else {
        init().then(() => {
            setLoaded(true);
        });
        return (
            <div>
                <h1>Loading...</h1>
            </div>
        );
    }
}

export default App;
