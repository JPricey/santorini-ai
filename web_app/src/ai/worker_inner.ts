async function initializeWasmWorker() {
    self.postMessage('startup');
    const wasm = await import('../../pkg/wasm_app');
    await wasm.default();

    const worker = new wasm.WasmApp();

    self.onmessage = async e => {
        const thinkingResponse = worker.computeNextMove(e.data[0], e.data[1]);
        self.postMessage(thinkingResponse);
    };

    self.postMessage('ready');
};

initializeWasmWorker();
