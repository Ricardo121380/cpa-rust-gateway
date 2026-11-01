/** Three offset tiles: one request fanned out to several providers. Colors come from the categorical palette. */
export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size * 0.85} viewBox="0 0 40 34" aria-hidden>
      <rect x="2" y="9" width="17" height="17" rx="5" fill="var(--c1)" transform="rotate(-12 10 17)" />
      <rect x="12" y="5" width="17" height="17" rx="5" fill="var(--c3)" opacity=".9" transform="rotate(8 20 13)" />
      <rect x="19" y="13" width="17" height="17" rx="5" fill="var(--c6)" opacity=".88" transform="rotate(-4 27 21)" />
    </svg>
  );
}
