// Glass sheet — modal layer (exempt from the 3-chrome-pane budget, but see the
// ONE-scrim budget below: the scrim owns the only full-viewport backdrop-filter
// in the app). Material and motion live in src/design/modal.css.
//
// Deliberately no backdrop-close option for reveal-once flows: closing is an
// explicit, understood action. Focus moves into the sheet on open, Tab is
// trapped inside it, and focus returns to the opener on close. Escape closes
// only when `onEscape` is provided.
import { createContext, useCallback, useContext, useEffect, useId, useRef, useState, type ButtonHTMLAttributes, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useBlocker } from "react-router-dom";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]),' +
  ' textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

let openSheets = 0;

type SheetCloseReason = "close" | "escape" | "route";
type SheetLayout = "form" | "confirm" | "inspector";
type SheetTone = "default" | "danger" | "success";

const SheetDismissContext = createContext<((afterDismiss?: () => void) => void) | undefined>(undefined);

function isLegacyDismissTarget(target: Element): target is HTMLButtonElement | HTMLAnchorElement {
  if (target instanceof HTMLButtonElement) return /^(取消|关闭|返回)$/.test(target.textContent?.trim() ?? "");
  return target instanceof HTMLAnchorElement && /^(#|\/)/.test(target.getAttribute("href") ?? "");
}

/**
 * A close-like action inside a Sheet. `onDismiss` is for an in-dialog back
 * transition (for example, a credential editor returning to its inspector):
 * it runs only after the user has accepted the same discard decision used by
 * Cancel, Escape and the header close button.
 */
export function SheetDismissButton({ children, type = "button", onClick, onDismiss, ...props }: Readonly<ButtonHTMLAttributes<HTMLButtonElement> & { onDismiss?: () => void }>) {
  const requestClose = useContext(SheetDismissContext);
  return <button {...props} type={type} data-sheet-dismiss="true" onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) requestClose?.(onDismiss); }}>{children}</button>;
}

export function Sheet({
  title,
  description,
  children,
  footer,
  onEscape,
  onBeforeDismiss,
  layout = "form",
  tone = "default",
  busy = false,
  isDirty = false,
  blockNavigation = false,
  guardUnsaved = true,
}: Readonly<{
  title: string;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  onEscape?: (() => void) | undefined;
  /**
   * An opt-in transaction boundary for flows with a live remote session. It
   * runs after a dirty-form discard choice but before a sheet is closed or a
   * blocked route is admitted. Returning false (or rejecting) keeps the
   * workspace and its actionable channel error in place.
   */
  onBeforeDismiss?: (() => boolean | Promise<boolean>) | undefined;
  layout?: SheetLayout;
  tone?: SheetTone;
  busy?: boolean;
  isDirty?: boolean;
  /** A live remote session has to run its opt-in dismissal cleanup on POP. */
  blockNavigation?: boolean;
  guardUnsaved?: boolean;
}>) {
  const panelRef = useRef<HTMLDivElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const keepEditingRef = useRef<HTMLButtonElement>(null);
  const discardFocusRef = useRef<HTMLElement | null>(null);
  const lastContentFocusRef = useRef<HTMLElement | null>(null);
  const deferredDismissRef = useRef<(() => void) | undefined>(undefined);
  const pendingFinishRef = useRef<{ reason: SheetCloseReason; deferred: (() => void) | undefined } | undefined>(undefined);
  const acceptedRouteCleanupRef = useRef(false);
  const dirtyRef = useRef(false);
  const [discardReason, setDiscardReason] = useState<SheetCloseReason>();
  const [dismissing, setDismissing] = useState(false);
  const titleId = useId();
  const descriptionId = useId();
  const hasUnsaved = useCallback(() => guardUnsaved && (dirtyRef.current || isDirty) && !!panelRef.current?.querySelector("form"), [guardUnsaved, isDirty]);
  // A pending irreversible write/reveal also blocks route admission. It is not
  // merely an unsaved form: the receipt may replace that form before the write
  // returns, and a queued Back navigation must never consume a one-time value.
  const effectiveBusy = busy || dismissing;
  const shouldBlockRoute = useCallback(() => effectiveBusy || blockNavigation || hasUnsaved(), [blockNavigation, effectiveBusy, hasUnsaved]);
  const blocker = useBlocker(shouldBlockRoute);
  const completeDismiss = useCallback((reason: SheetCloseReason, deferred: (() => void) | undefined) => {
    if (reason === "route" && blocker.state === "blocked") {
      // The router owns the accepted POP. Calling a parent close here can
      // unmount its blocker before the router replays the destination.
      blocker.proceed();
      acceptedRouteCleanupRef.current = false;
      return;
    }
    if (deferred !== undefined) queueMicrotask(deferred);
    else onEscape?.();
  }, [blocker, onEscape]);
  const finishClose = useCallback((reason: SheetCloseReason) => {
    void (async () => {
      if (onBeforeDismiss !== undefined) {
        acceptedRouteCleanupRef.current = reason === "route";
        setDismissing(true);
        let canDismiss = false;
        try {
          canDismiss = await onBeforeDismiss();
        } catch {
          canDismiss = false;
        }
        if (!canDismiss) {
          // A rejected POP must not be retried by the blocker effect after a
          // channel reports cancellation failure. The operator can retry an
          // explicit Cancel action once the actionable error is visible.
          if (blocker.state === "blocked") blocker.reset();
          acceptedRouteCleanupRef.current = false;
          setDismissing(false);
          return;
        }
      }
      const deferred = deferredDismissRef.current;
      deferredDismissRef.current = undefined;
      dirtyRef.current = false;
      setDiscardReason(undefined);
      if (onBeforeDismiss !== undefined) {
        // The cancellation request changed the effective blocker condition.
        // Let React Router observe that committed state before a parent close
        // changes a query parameter or otherwise navigates.
        pendingFinishRef.current = { reason, deferred };
        setDismissing(false);
        return;
      }
      completeDismiss(reason, deferred);
    })();
  }, [completeDismiss, onBeforeDismiss]);

  useEffect(() => {
    if (dismissing) return;
    const pending = pendingFinishRef.current;
    if (pending === undefined) return;
    pendingFinishRef.current = undefined;
    completeDismiss(pending.reason, pending.deferred);
  }, [completeDismiss, dismissing]);
  const requestClose = useCallback((reason: SheetCloseReason = "close", afterDismiss?: () => void) => {
    if (effectiveBusy || pendingFinishRef.current !== undefined) return;
    if (hasUnsaved()) {
      deferredDismissRef.current = afterDismiss;
      const active = document.activeElement;
      discardFocusRef.current = lastContentFocusRef.current?.isConnected
        ? lastContentFocusRef.current
        : active instanceof HTMLElement && panelRef.current?.contains(active) ? active : null;
      setDiscardReason(reason);
      return;
    }
    deferredDismissRef.current = afterDismiss;
    finishClose(reason);
  }, [effectiveBusy, finishClose, hasUnsaved]);

  useEffect(() => {
    if (blocker.state !== "blocked") return;
    if (dismissing) {
      if (!acceptedRouteCleanupRef.current) blocker.reset();
      return;
    }
    if (pendingFinishRef.current !== undefined) return;
    // A route attempt while an irreversible write/reveal is pending must not
    // become a latent navigation that fires after the durable receipt appears.
    if (effectiveBusy) { blocker.reset(); return; }
    requestClose("route");
  }, [blocker, blocker.state, dismissing, effectiveBusy, requestClose]);

  useEffect(() => {
    if (discardReason !== undefined) keepEditingRef.current?.focus();
  }, [discardReason]);

  // A durable mutation/reveal must never remain obscured by a stale discard
  // prompt created in the narrow interval before React observed `busy`.
  useEffect(() => {
    if (effectiveBusy && discardReason !== undefined) setDiscardReason(undefined);
  }, [effectiveBusy, discardReason]);

  const cancelDiscard = useCallback(() => {
    if (blocker.state === "blocked") blocker.reset();
    deferredDismissRef.current = undefined;
    setDiscardReason(undefined);
    queueMicrotask(() => (discardFocusRef.current?.isConnected ? discardFocusRef.current : panelRef.current)?.focus());
  }, [blocker]);

  useEffect(() => {
    const handler = (event: BeforeUnloadEvent) => { if (hasUnsaved()) { event.preventDefault(); event.returnValue = ""; } };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [hasUnsaved]);


  useEffect(() => {
    // StrictMode reruns the effect after focus has entered the panel. Preserve
    // the original opener instead of replacing it with the panel's first button.
    openerRef.current ??= document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const opener = openerRef.current;
    (panelRef.current?.querySelector<HTMLElement>(FOCUSABLE) ?? panelRef.current)?.focus();

    openSheets += 1;
    document.documentElement.classList.add("has-sheet");
    document.getElementById("app")?.setAttribute("inert", "");
    if (import.meta.env.DEV && openSheets > 1) {
      console.error(
        `modal budget exceeded: ${openSheets} scrims mounted (max 1 — a scrim is` +
          " the only full-viewport backdrop-filter in the app)",
      );
    }

    return () => {
      openSheets -= 1;
      queueMicrotask(() => {
        if (openSheets === 0) {
          document.documentElement.classList.remove("has-sheet");
          document.getElementById("app")?.removeAttribute("inert");
        }
        // A replacement dialog owns focus. Never revive secret-bearing DOM just
        // to animate an exit, and only return focus when no dialog replaced it.
        if (openSheets === 0 && !document.querySelector('[role="dialog"]') && opener !== null && opener.isConnected) {
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
        if (discardReason !== undefined) cancelDiscard();
        else requestClose("escape");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [cancelDiscard, discardReason, onEscape, requestClose]);

  /** aria-modal alone does not stop Tab from walking into the page behind. */
  const trapTab = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key !== "Tab" || panelRef.current === null) {
      return;
    }
    const focusRoot = discardReason === undefined ? panelRef.current : panelRef.current.querySelector<HTMLElement>(".sheet-discard");
    if (focusRoot === null) return;
    const items = [...focusRoot.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (el) => el.getClientRects().length > 0 && !el.closest("[inert], [aria-hidden=\"true\"]"),
    );
    const first = items.at(0);
    const last = items.at(-1);
    if (first === undefined || last === undefined) {
      event.preventDefault();
      panelRef.current.focus();
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
    <div className="sheet-backdrop" data-modal="scrim" data-layout={layout} data-tone={tone} role="presentation">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description === undefined ? undefined : descriptionId}
        tabIndex={-1}
        ref={panelRef}
        onKeyDown={trapTab}
        onClickCapture={event => {
          // Compatibility for legacy callers. New flows use SheetDismissButton;
          // this path defers the original handler until discard is accepted.
          const target = event.target instanceof Element ? event.target.closest("button,a") : null;
          if (target === null || target.closest("[data-sheet-dismiss], .sheet-discard") || !isLegacyDismissTarget(target)) return;
          if (effectiveBusy) { event.preventDefault(); event.stopPropagation(); return; }
          if (!hasUnsaved()) return;
          event.preventDefault();
          event.stopPropagation();
          requestClose("close", () => target.click());
        }}
        onFocusCapture={event => {
          const target = event.target;
          if (target instanceof HTMLElement && !target.closest("[data-sheet-dismiss], .sheet-close")) lastContentFocusRef.current = target;
        }}
        onInputCapture={() => { dirtyRef.current = true; }}
        onChangeCapture={() => { dirtyRef.current = true; }}
      >
        <div className="sheet-panel">
          <header className="sheet-heading" inert={discardReason !== undefined} aria-hidden={discardReason !== undefined}><div><h2 id={titleId}>{title}</h2>{description === undefined ? null : <p id={descriptionId} className="sheet-description">{description}</p>}</div>{onEscape === undefined ? null : <button type="button" className="secondary sheet-close" aria-label="关闭面板" disabled={effectiveBusy} onClick={() => requestClose("close")}>×</button>}</header>
          <SheetDismissContext.Provider value={(afterDismiss) => requestClose("close", afterDismiss)}><div className="sheet-body" inert={discardReason !== undefined} aria-hidden={discardReason !== undefined}>{children}</div>{footer === undefined ? null : <footer className="sheet-footer" inert={discardReason !== undefined} aria-hidden={discardReason !== undefined}>{footer}</footer>}</SheetDismissContext.Provider>
          {discardReason === undefined ? null : <section className="sheet-discard" role="alertdialog" aria-labelledby={`${titleId}-discard`} aria-describedby={`${descriptionId}-discard`}><h3 id={`${titleId}-discard`}>放弃未保存的修改？</h3><p id={`${descriptionId}-discard`} className="muted">继续关闭将放弃本次输入。</p><div className="sheet-actions"><button ref={keepEditingRef} type="button" className="secondary" onClick={cancelDiscard}>继续编辑</button><button type="button" className={tone === "danger" ? "danger" : undefined} onClick={() => finishClose(discardReason)}>放弃修改</button></div></section>}
        </div>
      </div>
    </div>,
    document.body,
  );
}
