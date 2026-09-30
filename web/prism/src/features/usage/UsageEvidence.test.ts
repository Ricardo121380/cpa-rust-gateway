import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { en } from "../../i18n/en";
import { provenanceLabel, UsageEvidence } from "./UsageEvidence";

it("keeps legacy evidence unknown even when numeric counts and prices are exact", () => {
  const html = renderToStaticMarkup(createElement(UsageEvidence));
  expect(html).toContain("来源未知");
  expect(html).toContain("输入与缓存关系未知");
  expect(html).not.toContain("实测");
});
it("distinguishes estimated usage from pricing and preserves input accounting", () => {
  const html = renderToStaticMarkup(createElement(UsageEvidence, { provenance: "estimated", inputAccounting: "inclusive" }));
  expect(html).toContain("估算用量");
  expect(html).toContain("输入已包含缓存");
});
it("formats source evidence with the provided locale, including legacy unknown values", () => {
  expect(provenanceLabel("estimated", en.usageEvidence)).toBe("Estimated usage");
  expect(provenanceLabel("measured", en.usageEvidence)).toBe("Upstream measured");
  expect(provenanceLabel("mixed", en.usageEvidence)).toBe("Mixed sources");
  expect(provenanceLabel(undefined, en.usageEvidence)).toBe("Unknown source");
});
