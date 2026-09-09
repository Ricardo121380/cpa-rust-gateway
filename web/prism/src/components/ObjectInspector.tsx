import type { ReactNode } from "react";
import { Sheet } from "./Sheet";

/** Uses explicitly selected, already-loaded public fields, never arbitrary
 *  serialization or another detail fetch. Secrets must not enter this view. */
export function ObjectInspector({ title, scope, facts, children, onClose }: Readonly<{
  title: string;
  scope: string;
  facts: ReadonlyArray<readonly [string, ReactNode]>;
  children?: ReactNode;
  onClose: () => void;
}>) {
  return <Sheet title={title} layout="inspector" onEscape={onClose}>
    <p className="scope-row">{scope}</p>
    <dl className="fact-grid">{facts.map(([label, value]) =>
      <div key={label}><dt>{label}</dt><dd>{value ?? "—"}</dd></div>)}</dl>
    {children}
  </Sheet>;
}
