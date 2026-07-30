// Subscribe to a media query and re-render when it flips. The settings page
// reports OS accessibility preferences, and those can change while the panel is
// open (the user goes to System Settings to check), so a one-shot read at mount
// would go stale and silently misreport.
import { useEffect, useState } from "react";

export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);

  useEffect(() => {
    const list = window.matchMedia(query);
    setMatches(list.matches); // the query may have changed since the last render
    const onChange = (event: MediaQueryListEvent): void => setMatches(event.matches);
    list.addEventListener("change", onChange);
    return () => list.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}
