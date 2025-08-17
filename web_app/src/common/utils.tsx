import { isDeepEqual } from 'remeda';

export function sigmoid(x: number) {
    return 1 / (1 + Math.exp(-x));
}

export async function setTimeoutAsync(timeout: number): Promise<void> {
    return new Promise((resolve) => {
        window.setTimeout(() => {
            resolve();
        }, timeout);
    });
}

export function capitalizeFirstLetter(value: string): string {
    if (value.length === 0) {
        return value;
    }
    return value.charAt(0).toUpperCase() + value.slice(1);
}

export function isListSubset(a: Array<any>, b: Array<any>): boolean {
    if (a.length > b.length) {
        return false;
    }

    for (let i = 0; i < a.length; i++) {
        if (!isDeepEqual(a[i], b[i])) {
            return false;
        }
    }

    return true;
}

export function isListDeepContain<T>(a: Array<T>, b: T) {
    for (const item of a) {
        if (isDeepEqual(item, b)) {
            return true;
        }
    }

    return false;
}

export type DeferredPromise<T> = {
    promise: Promise<T>,
    resolve: (x: T) => void,
    reject: () => void,
};
export function createDeferredPromise<T>(): DeferredPromise<T> {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return {
        promise: promise as Promise<T>,
        resolve: resolve as any,
        reject: reject as any,
    };
}

export function assertUnreachable(x: never): never {
    throw new Error(`Didn't expect to get here: ${JSON.stringify(x)}`);
}

