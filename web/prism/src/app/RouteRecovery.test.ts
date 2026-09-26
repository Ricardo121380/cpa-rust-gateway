import {it,expect} from "vitest";
import {renderFailureReference} from "./RouteRecovery";
it("returns a stable opaque reference without the thrown message, stack or object contents",()=>{
  const error=new Error("secret-containing upstream exception");
  expect(renderFailureReference(error)).toMatch(/^UI-[0-9a-f]{8}$/u);
  expect(renderFailureReference(error)).toBe(renderFailureReference(error));
  expect(renderFailureReference({secret:"never serialize"})).not.toContain("secret");
});
