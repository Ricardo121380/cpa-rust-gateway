// HashRouter receives browser Back from window history events. The protected
// entry's state is saved while the Sheet is busy, so a rejected traversal can
// recreate that exact router history state rather than copying the destination.
let expectedHref: string | undefined;
let expectedState: unknown;

export function setBusySheetHistoryGuard(busy: boolean): void {
  expectedHref = busy ? window.location.href : undefined;
  expectedState = busy ? window.history.state : undefined;
}

function restoreBusyLocation(event: Event): void {
  if (expectedHref === undefined || window.location.href === expectedHref) return;
  event.stopImmediatePropagation();
  // Pushing after a Back discards the rejected forward branch and makes a new
  // protected entry. Preserve the saved source state (including HashRouter's
  // index/key) instead of reading the destination's state after the pop.
  window.history.pushState(expectedState, "", expectedHref);
}

if (typeof window !== "undefined") {
  window.addEventListener("popstate", restoreBusyLocation, { capture: true });
  window.addEventListener("hashchange", restoreBusyLocation, { capture: true });
}
