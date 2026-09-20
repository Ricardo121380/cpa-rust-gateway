import { createContext, useContext, useMemo, useRef, type ReactNode } from "react";

type Admission = (next: () => void) => void;
type Boundary = { request: Admission; register: (owner: Admission) => () => void };
const Context = createContext<Boundary | undefined>(undefined);

/** Local transitions and the dock ask the same operation that owns navigation. */
export function OperationBoundary({children}: {children: ReactNode}) {
  const owner = useRef<Admission | undefined>(undefined);
  const value = useMemo<Boundary>(() => ({
    request: next => owner.current ? owner.current(next) : next(),
    register: next => {
      owner.current = next;
      return () => { if (owner.current === next) owner.current = undefined; };
    },
  }), []);
  return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function useOperationBoundary() {
  const boundary = useContext(Context);
  if (!boundary) throw new Error("OperationBoundary is required");
  return boundary;
}
