import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ReadStatus } from "./ReadStatus";

describe("management read feedback", () => {
  it("keeps the previous snapshot visible and prevents duplicate retries while fetching", () => {
    const html = renderToStaticMarkup(createElement(ReadStatus, {
      pending: false, fetching: true, hasData: true, dataUpdatedAt: 1_700_000_000_000,
      error: { kind: "network", code: "network_error", message: "connection lost" },
      retry: () => undefined,
    }));
    expect(html).toContain("刷新失败");
    expect(html).toContain("上次成功读取");
    expect(html).toContain("并非最新状态");
    expect(html).toContain("disabled");
    expect(html).toContain("读取中");
  });
  it("does not infer that a projection is disabled from an unavailable response", () => {
    const html = renderToStaticMarkup(createElement(ReadStatus, {
      pending: false, hasData: false,
      error: { kind: "unavailable", code: "management_capacity_exceeded", message: "try later", status: 503 },
      retry: () => undefined,
    }));
    expect(html).toContain("management_capacity_exceeded");
    expect(html).toContain("暂时无法读取");
    expect(html).not.toContain("未启用");
  });
});
