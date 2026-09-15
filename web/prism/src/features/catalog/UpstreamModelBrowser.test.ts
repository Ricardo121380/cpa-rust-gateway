import { describe, expect, it } from "vitest";
import { selectedCatalogCredential, selectedCatalogEndpoint } from "./UpstreamModelBrowser";

describe("upstream catalog selections", () => {
  it("never turns the first endpoint into an implicit directory target", () => {
    const endpoints=[{id:"first"},{id:"second"}];
    expect(selectedCatalogEndpoint("",endpoints)).toBeUndefined();
    expect(selectedCatalogEndpoint("second",endpoints)).toEqual({id:"second"});
  });

  it("keeps directory evidence scoped to an explicitly selected account", () => {
    const accounts=[{credential_id:"one"},{credential_id:"two"}];
    expect(selectedCatalogCredential("",accounts)).toBeUndefined();
    expect(selectedCatalogCredential("two",accounts)).toEqual({credential_id:"two"});
  });
});
