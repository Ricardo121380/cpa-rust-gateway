import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it } from "vitest";
import { ProcessingStatus, type BillingProcessingStatus } from "./ProcessingStatus";

function render(overrides: Partial<BillingProcessingStatus>) {
  const client = new QueryClient();
  client.setQueryData(["billing-processing"], {
    state: "current", observed_at_ms: Date.now(), source_ordinal: 10,
    checkpoint_ordinal: 10, checkpoint_updated_at_ms: Date.now(),
    unresolved_failures: 0, quarantined_failures: 0, failure_code: null, ...overrides,
  });
  const html = renderToStaticMarkup(createElement(QueryClientProvider, {client}, createElement(ProcessingStatus, {compact: true})));
  client.clear();
  return html;
}

it("keeps stale or backlogged observations visible even when the last state was current", () => {
  expect(render({})).toContain('class="processing-fold"');
  const stale = render({observed_at_ms: Date.now() - 61_000});
  expect(stale).toContain("处理观测已超过 1 分钟");
  expect(stale).not.toContain('class="processing-fold"');
  const lag = render({source_ordinal: 5000, checkpoint_ordinal: 1});
  expect(lag).toContain("待处理序号跨度较大");
  expect(lag).not.toContain('class="processing-fold"');
});
