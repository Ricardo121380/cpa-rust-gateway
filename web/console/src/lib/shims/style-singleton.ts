// CSP shim for react-style-singleton (used by react-remove-scroll inside Radix overlays).
// The gateway serves style-src 'self', so runtime <style> injection is blocked. The scroll-lock rules it
// would inject live statically in globals.css under body[data-scroll-locked]; the attribute itself is
// still toggled by react-remove-scroll-bar through setAttribute, which CSP allows.
export const stylesheetSingleton = () => ({ add: () => {}, remove: () => {} });
export const styleHookSingleton = () => () => {};
export const styleSingleton = () => () => null;
