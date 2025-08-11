// import init, { WasmApp } from "../../pkg/wasm_app";

async function initializeWasmWorker() {
    self.postMessage('startup');
    const wasm = await import('../../pkg/wasm_app'); // adjust path as needed
    await wasm.default();

    const worker = new wasm.WasmApp();

    self.onmessage = async e => {
        // console.log(new Date().toISOString(), 'Message received from main thread: ', e.data);
        const thinkingResponse = worker.computeNextMove(e.data[0], e.data[1]);
        self.postMessage(thinkingResponse);
    };

    self.postMessage('ready');
};

initializeWasmWorker();
