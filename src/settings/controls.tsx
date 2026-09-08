import type { ReactNode, Ref } from "react";

export function Toggle({ checked, disabled = false, label, onChange }: { checked: boolean; disabled?: boolean; label: string; onChange: (next: boolean) => void }) {
  return <label className="toggle-row" data-disabled={disabled}><span>{label}</span><input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span className="toggle" aria-hidden="true" /></label>;
}

export function SettingGroup({ title, description, children, id, sectionRef, focusable = false }: { title: string; description: string; children: ReactNode; id?: string; sectionRef?: Ref<HTMLElement>; focusable?: boolean }) {
  const headingId = id ? `${id}-heading` : undefined;
  return <section className="setting-group" id={id} ref={sectionRef} tabIndex={focusable ? -1 : undefined} aria-labelledby={headingId}><header><h2 id={headingId}>{title}</h2><p>{description}</p></header><div className="setting-controls">{children}</div></section>;
}

export const pointerSpeedOptions = [5, 25, 50, 75, 100] as const;
export const pointerSpeedValues = Array.from({ length: 45 }, (_, index) => (index + 1) * 5);
export const repeatIntervalOptions = [100, 250, 500, 1000] as const;
export const keyRepeatDelayOptions = [
  { value: 0, label: "None" },
  { value: 250, label: "Short" },
  { value: 500, label: "Medium" },
  { value: 1000, label: "Long" },
] as const;
export const accelerationOptions = [
  { value: 0, label: "Off" },
  { value: 500, label: "Short" },
  { value: 1000, label: "Medium" },
  { value: 2000, label: "Long" },
] as const;
export const dwellDelayOptions = [500, 1000, 1500, 2000, 3000, 4000, 5000, 6000, 7000, 8000] as const;

export function movementValue(base: number, scale: number) {
  const value = Math.min(50, Math.max(1, Math.round((base * scale / 100) * 2) / 2));
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

export function OptionGroup<T extends string | number>({ legend, options, value, onChange, disabled, columns, children }: {
  legend: string;
  options: ReadonlyArray<{ value: T; label: ReactNode }>;
  value: T;
  onChange: (next: T) => void;
  disabled: boolean;
  columns?: "three" | "four" | "five";
  children?: ReactNode;
}) {
  return <fieldset disabled={disabled}><legend>{legend}</legend><div className={columns ? `segmented compact ${columns}` : "segmented compact"}>
    {options.map((option) => <button type="button" key={option.value} aria-pressed={value === option.value} onClick={() => onChange(option.value)}>{option.label}</button>)}
  </div>{children}</fieldset>;
}

export function secondsOptions<T extends number>(values: readonly T[]) {
  return values.map((value) => ({ value, label: <>{value / 1000}s</> }));
}

export const overlayVisibilityOptions = [
  { value: "onInput", label: "On input" },
  { value: "whileControlling", label: "While controlling" },
] as const;

export const overlaySizeOptions = (["small", "medium", "large"] as const)
  .map((value) => ({ value, label: value[0].toUpperCase() + value.slice(1) }));
