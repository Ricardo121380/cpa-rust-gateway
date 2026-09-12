// Glass sheet — modal layer (exempt from the 3-chrome-pane budget, but see the
// ONE-scrim budget below: the scrim owns the only full-viewport backdrop-filter
// in the app). Material and motion live in src/design/modal.css.
//
// Deliberately no backdrop-close option for reveal-once flows: closing is an
// explicit, understood action. Focus moves into the sheet on open, Tab is
// trapped inside it, and focus returns to the opener on close. Escape closes
// only when `onEscape` is provided.
import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useSessionStore } from "../session/sessionStore";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]),' +
  ' textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** longest exit animation in modal.css (190ms) + slack for a dropped frame */
const EXIT_FALLBACK_MS = 700;

let openSheets = 0;

/** Exit motion for an element React has already unmounted.
 *  React has no exit hook and every call site closes by flipping its own state,
 *  so the only way to animate the way out without touching eight pages is to
 *  re-adopt the detached scrim, play `sheet-out` on it and drop it. The ghost is
 *  inert, aria-hidden and one z-index below the live layer, so if the next sheet
 *  opens in the same tick (issue -> reveal) the two crossfade instead of
 *  fighting. */
function playExit(scrim: HTMLElement): void {
  // Exit animation retains a detached DOM tree briefly. Secret values must not outlive close.
  for (const node of scrim.querySelectorAll(".reveal-key")) node.textContent = "";
  for (const input of scrim.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("input[type=password], textarea")) input.value = "";
  scrim.classList.add("sheet-ghost");
  scrim.setAttribute("aria-hidden", "true");
  scrim.setAttribute("inert", "");
  document.body.append(scrim);
  const drop = (): void => scrim.remove();
  scrim.addEventListener("animationend", (event) => {
    if (event.target === scrim) {
      drop();
    }
  });
  window.setTimeout(drop, EXIT_FALLBACK_MS);
}

export function Sheet({
  title,
  children,
  onEscape,
  layout = "form",
}: Readonly<{
  title: string;
  children: ReactNode;
  onEscape?: (() => void) | undefined;
  layout?: "form" | "inspector";
}>) {
  const scrimRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const scrim = scrimRef.current;
    const sessionGeneration = useSessionStore.getState().generation;
    // StrictMode reruns the effect after focus has entered the panel. Preserve
    // the original opener instead of replacing it with the panel's first button.
    openerRef.current ??= document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const opener = openerRef.current;
    (panelRef.current?.querySelector<HTMLElement>(FOCUSABLE) ?? panelRef.current)?.focus();

    openSheets += 1;
    if (import.meta.env.DEV && openSheets > 1) {
      console.error(
        `modal budget exceeded: ${openSheets} scrims mounted (max 1 — a scrim is` +
          " the only full-viewport backdrop-filter in the app)",
      );
    }

    return () => {
      openSheets -= 1;
      // Deferred by one microtask so a real unmount (node already detached) can
      // be told apart from StrictMode's simulated remount (node kept, effect
      // re-run) — otherwise every sheet would spawn a ghost the moment it opened
      // in development.
      queueMicrotask(() => {
        if (scrim === null || scrim.isConnected) {
          return;
        }
        // A lock must not reattach a reveal-once secret as an exit-animation
        // ghost. Animate only inside the same still-unlocked session.
        const session = useSessionStore.getState();
        if (session.unlocked && session.generation === sessionGeneration) playExit(scrim);
        if (opener !== null && opener.isConnected) {
          opener.focus();
        }
      });
    };
  }, []);

  useEffect(() => {
    if (onEscape === undefined) {
      return;
    }
    const handler = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        onEscape();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onEscape]);

  /** aria-modal alone does not stop Tab from walking into the page behind. */
  const trapTab = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key !== "Tab" || panelRef.current === null) {
      return;
    }
    const items = [...panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (el) => el.getClientRects().length > 0,
    );
    const first = items.at(0);
    const last = items.at(-1);
    if (first === undefined || last === undefined) {
      return;
    }
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === panelRef.current)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  };

  // Portalled to <body>: pages render their sheets inside the canvas, and the
  // canvas now carries the scroll-edge mask + its own scroll container — both
  // would clip a fixed modal painted inside its subtree.
  return createPortal(
    <div className="sheet-backdrop" data-modal="scrim" data-layout={layout} role="presentation" ref={scrimRef}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        ref={panelRef}
        onKeyDown={trapTab}
      >
        <div className="sheet-panel">
          <header className="sheet-heading"><h3>{title}</h3>{onEscape === undefined ? null : <button type="button" className="secondary" aria-label="关闭面板" onClick={onEscape}>×</button>}</header>
          {children}
        </div>
      </div>
    </div>,
    document.body,
  );
}
