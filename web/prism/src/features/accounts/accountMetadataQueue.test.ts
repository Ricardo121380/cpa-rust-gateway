import {describe, expect, it, vi} from "vitest";
import {createAccountMetadataQueue} from "./accountMetadataQueue";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>(done => { resolve = done; });
  return {promise, resolve};
}

describe("account metadata scheduling", () => {
  it("limits concurrent upstream observations without dropping other accounts", async () => {
    const read = createAccountMetadataQueue();
    const gates = Array.from({length: 5}, deferred);
    const started: number[] = [];
    const tasks = gates.map((gate, index) => read(new AbortController().signal, async () => {
      started.push(index);
      await gate.promise;
    }));
    expect(started).toEqual([0, 1]);
    gates[0]!.resolve();
    await tasks[0];
    expect(started).toEqual([0, 1, 2]);
    gates.forEach(gate => gate.resolve());
    await Promise.all(tasks);
    expect(started).toEqual([0, 1, 2, 3, 4]);
  });

  it("removes queued reads on session cancellation and releases failed slots", async () => {
    const read = createAccountMetadataQueue(1);
    const gate = deferred();
    const first = read(new AbortController().signal, () => gate.promise);
    const abort = new AbortController();
    const request = vi.fn();
    const queued = read(abort.signal, request);
    const rejected = expect(queued).rejects.toMatchObject({name: "AbortError"});
    abort.abort();
    await rejected;
    gate.resolve();
    await first;
    expect(request).not.toHaveBeenCalled();
    await expect(read(new AbortController().signal, async () => {throw new Error("offline");})).rejects.toThrow("offline");
    await expect(read(new AbortController().signal, async () => "recovered")).resolves.toBe("recovered");
  });
});
