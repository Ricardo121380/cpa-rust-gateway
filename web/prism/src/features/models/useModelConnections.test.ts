import { beforeEach, describe, expect, it, vi } from "vitest";
import { call } from "../../api/client";
import { readModelConnections } from "./useModelConnections";

vi.mock("../../api/client", () => ({ call: vi.fn() }));

describe("readModelConnections", () => {
  beforeEach(() => vi.mocked(call).mockReset());

  it("reads the bounded topology in order and retains a shared revision", async () => {
    vi.mocked(call).mockResolvedValueOnce({ revision: "rev-7", items: [{ id: "route-a" }], next_cursor: null })
      .mockResolvedValueOnce({ revision: "rev-7", items: [{ id: "candidate-a" }], next_cursor: null })
      .mockResolvedValueOnce({ revision: "rev-7", items: [{ id: "endpoint-a" }], next_cursor: null });

    await expect(readModelConnections()).resolves.toEqual({
      routes: [{ id: "route-a" }],
      candidates: [{ id: "candidate-a" }],
      endpoints: [{ id: "endpoint-a" }],
    });
    expect(vi.mocked(call).mock.calls.map(([operation]) => operation)).toEqual([
      "listRoutes",
      "listRouteCandidates",
      "listManagedEndpoints",
    ]);
  });
});
