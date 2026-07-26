// Controlled chips editor for string lists (hosts, tags, ports-as-text).
// Enter or comma commits the pending entry; clicking a chip removes it.
import { useState, type KeyboardEvent } from "react";

export function ChipsInput({
  value,
  onChange,
  placeholder,
  validate,
  mono = true,
}: Readonly<{
  value: readonly string[];
  onChange: (next: string[]) => void;
  placeholder?: string;
  /** returns an error message, or undefined when the entry is acceptable */
  validate?: (entry: string) => string | undefined;
  mono?: boolean;
}>) {
  const [pending, setPending] = useState("");
  const [error, setError] = useState<string | undefined>();

  function commit() {
    const entry = pending.trim();
    if (entry.length === 0) {
      return;
    }
    if (value.includes(entry)) {
      setPending("");
      return;
    }
    const problem = validate?.(entry);
    if (problem !== undefined) {
      setError(problem);
      return;
    }
    onChange([...value, entry]);
    setPending("");
    setError(undefined);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      commit();
    }
    if (event.key === "Backspace" && pending.length === 0 && value.length > 0) {
      onChange(value.slice(0, -1));
    }
  }

  return (
    <div className="chips">
      <div className="chips-row">
        {value.map((entry) => (
          <button
            key={entry}
            type="button"
            className={mono ? "chip mono" : "chip"}
            title="点击移除"
            onClick={() => onChange(value.filter((candidate) => candidate !== entry))}
          >
            {entry} ×
          </button>
        ))}
        <input
          className={mono ? "mono" : undefined}
          value={pending}
          placeholder={placeholder}
          onChange={(event) => {
            setPending(event.target.value);
            setError(undefined);
          }}
          onKeyDown={onKeyDown}
          onBlur={commit}
        />
      </div>
      {error !== undefined ? <p className="chips-error">{error}</p> : null}
    </div>
  );
}
