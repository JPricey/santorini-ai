import type { PlayerAction } from "../common/api";
import { createDeferredPromise, type DeferredPromise } from "../common/utils";

export type SearchResultMeta = {
    score: number,
    calculated_depth: number,
    nodes_visited: number,
    action_str: string,
    actions: Array<PlayerAction>
};

export type SearchResult = {
    original_str: string,
    start_state: string,
    next_state: string,
    trigger: string,
    meta: SearchResultMeta,
}

export class AiWorker {
    private worker: Worker;
    private isReadyPromise: DeferredPromise<void>;
    private promiseMap: Map<string, DeferredPromise<SearchResult>>;

    constructor() {
        this.isReadyPromise = createDeferredPromise();
        this.promiseMap = new Map();
        this.worker = new Worker(new URL('./worker_inner.ts', import.meta.url))

        this.worker.onmessage = (ev) => {
            if (ev.data === 'ready') {
                this.isReadyPromise.resolve();
            } else {
                const key = ev.data.original_str;
                const entry = this.promiseMap.get(key);
                if (entry) {
                    entry.resolve(ev.data);
                }
            }
        };
    }

    async getAiResult(fen: string, duration: number): Promise<SearchResult> {
        await this.isReadyPromise.promise;

        const oldPromise = this.promiseMap.get(fen);
        if (oldPromise) {
            return oldPromise.promise;
        }

        const resultPromise = createDeferredPromise<SearchResult>();
        this.promiseMap.set(fen, resultPromise);

        this.worker.postMessage([fen, duration]);

        return resultPromise.promise;
    }

    clearMap() {
        this.promiseMap.clear();
    }
}
