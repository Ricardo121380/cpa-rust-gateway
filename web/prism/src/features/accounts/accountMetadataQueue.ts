// Reserve headroom in the server's four metadata slots for interactive reads.
// Waiting reads belong to their query's lifetime and must not survive logout.
export function createAccountMetadataQueue(concurrency = 2) {
  let active = 0;
  const waiting: Array<() => void> = [];
  return async function read<T>(signal: AbortSignal, request: () => Promise<T>): Promise<T> {
    signal.throwIfAborted();
    if (active >= concurrency) {
      await new Promise<void>((resolve, reject) => {
        const start = () => {
          signal.removeEventListener("abort", cancel);
          active++;
          resolve();
        };
        const cancel = () => {
          const index = waiting.indexOf(start);
          if (index >= 0) waiting.splice(index, 1);
          reject(signal.reason);
        };
        waiting.push(start);
        signal.addEventListener("abort", cancel, {once: true});
      });
    } else {
      active++;
    }
    try {
      signal.throwIfAborted();
      return await request();
    } finally {
      active--;
      waiting.shift()?.();
    }
  };
}

export const readAccountMetadata = createAccountMetadataQueue();
