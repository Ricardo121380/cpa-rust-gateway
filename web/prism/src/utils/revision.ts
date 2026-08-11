// Revision pipeline model (docs/08 §2.2): every successful management response
// carries `ETag: "rev-N"` holding the NEXT expected revision; mutations echo it
// via If-Match. 409 means a concurrent writer advanced it — refetch, never replay.

const REVISION_PATTERN = /^"?(rev-(?:0|[1-9][0-9]*))"?$/u;

export function parseRevisionToken(etag: string | null): string | undefined {
  if (etag === null) {
    return undefined;
  }
  const match = REVISION_PATTERN.exec(etag.trim());
  return match?.[1];
}

export type VersionContext = Readonly<{
  configVersionId: string;
  revision: string;
  status: "draft" | "active" | "archived";
}>;

export function advanceRevision(
  context: VersionContext,
  etag: string | null,
): VersionContext {
  const next = parseRevisionToken(etag);
  if (next === undefined || next === context.revision) {
    return context;
  }
  return { ...context, revision: next };
}

export function isEditable(context: VersionContext | undefined): boolean {
  return context?.status === "draft";
}
