import { describe, expect, it } from "vitest";
import { samePublicModel, sameRoute } from "./advancedRoutingModel";
import type { PublicModel, RouteListItem } from "./model";

describe("advanced model and route baselines", () => {
  const model: PublicModel = { id: "pm", model_name: "Exact/Model", display_name: "Exact/Model", status: "active", capabilities: { streaming: true, vision: false } };
  it("compares model fields independent of object order while retaining explicit false", () => {
    expect(samePublicModel({ ...model, capabilities: { vision: false, streaming: true } }, model)).toBe(true);
    expect(samePublicModel({ ...model, capabilities: { streaming: true } }, model)).toBe(false);
    expect(samePublicModel({ ...model, capabilities: { streaming: true, vision: true } }, model)).toBe(false);
    expect(samePublicModel({ ...model, model_name: "exact/model" }, model)).toBe(false);
  });
  const route: RouteListItem = { id: "route", public_model_id: "pm", policy: "smooth_weighted_round_robin", max_attempts: 3, bootstrap_timeout_ms: 1000 };
  it("checks the route owner and operational parameters", () => {
    expect(sameRoute({ ...route }, route)).toBe(true);
    expect(sameRoute({ ...route, public_model_id: "other" }, route)).toBe(false);
    expect(sameRoute({ ...route, max_attempts: 4 }, route)).toBe(false);
  });
});
