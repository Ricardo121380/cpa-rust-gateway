// Glass sheet — modal layer (exempt from the 3-chrome-pane budget).
// Deliberately no backdrop-close option for reveal-once flows: closing is an
// explicit, understood action. Focus moves into the sheet on open; Escape
// closes only when `onEscape` is provided.
import { useEffect, useRef, type ReactNode } from "react";
import { GlassSurface } from "./glass/GlassSurface";

export function Sheet({
  title,
  children,
  onEscape,
}: Readonly<{
  title: string;
  children: ReactNode;
  onEscape?: (() => void) | undefined;
}>) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    panelRef.current?.querySelector<HTMLElement>("button, input, [tabindex]")?.focus();
    if (onEscape === undefined) {
      return;
    }
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onEscape();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onEscape]);

  return (
    <div className="sheet-backdrop" role="presentation">
      <div role="dialog" aria-modal="true" aria-label={title} ref={panelRef}>
        <GlassSurface className="sheet-panel" layer="modal">
          <h3>{title}</h3>
          {children}
        </GlassSurface>
      </div>
    </div>
  );
}
