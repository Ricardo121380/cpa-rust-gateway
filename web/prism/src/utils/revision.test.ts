import { describe, expect, it } from "vitest";
import { advanceRevision, isEditable, parseRevisionToken } from "./revision";

describe("parseRevisionToken", () => {
  it("accepts canonical quoted and bare tokens", () => {
    expect(parseRevisionToken('"rev-42"')).toBe("rev-42");
    expect(parseRevisionToken("rev-0")).toBe("rev-0");
  });

  it("rejects non-canonical forms", () => {
    expect(parseRevisionToken('"rev-007"')).toBeUndefined();
    expect(parseRevisionToken("rev-")).toBeUndefined();
    expect(parseRevisionToken("W/\"rev-1\"")).toBeUndefined();
    expect(parseRevisionToken(null)).toBeUndefined();
    expect(parseRevisionToken('"rev-1')).toBeUndefined();
    expect(parseRevisionToken('rev-1"')).toBeUndefined();
  });
});

describe("advanceRevision", () => {
  const context = {
    configVersionId: "draft-a",
    revision: "rev-4",
    status: "draft",
  } as const;

  it("advances to the ETag revision", () => {
    expect(advanceRevision(context, '"rev-5"').revision).toBe("rev-5");
  });

  it("keeps context on missing or identical ETag", () => {
    expect(advanceRevision(context, null)).toBe(context);
    expect(advanceRevision(context, '"rev-4"')).toBe(context);
  });
  it("ignores older responses and compares beyond Number's integer precision", () => {
    expect(advanceRevision(context, '"rev-3"')).toBe(context);
    const large = { ...context, revision: "rev-9007199254740992" };
    expect(advanceRevision(large, '"rev-9007199254740993"').revision).toBe("rev-9007199254740993");
  });
});

describe("isEditable", () => {
  it("only drafts are editable", () => {
    expect(isEditable({ configVersionId: "v", revision: "rev-1", status: "draft" })).toBe(true);
    expect(isEditable({ configVersionId: "v", revision: "rev-1", status: "active" })).toBe(false);
    expect(isEditable(undefined)).toBe(false);
  });
});
