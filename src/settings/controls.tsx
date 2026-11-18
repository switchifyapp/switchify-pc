import { Button, Input } from "../ui/controls";
import type { AppSettings } from "../types";
import { useId, useState, type ReactNode, type Ref } from "react";
import type { SettingId } from "../find/index";
import { formatSeconds, stepTime, type TimeRange } from "./timing";

// With labelledBy, the row's own heading (and optional description) names
// the switch, which then sits alone at the end of the row.
type ToggleName = { label: string; labelledBy?: never; describedBy?: never } | { label?: never; labelledBy: string; describedBy?: string };
export function Toggle({ checked, disabled = false, label, labelledBy, describedBy, onChange }: { checked: boolean; disabled?: boolean; onChange: (next: boolean) => void } & ToggleName) {
  return <label className={labelledBy ? "toggle-row toggle-bare" : "toggle-row"} data-disabled={disabled}>
    {!labelledBy && <span>{label}</span>}
    <Input type="checkbox" checked={checked} disabled={disabled} aria-labelledby={labelledBy} aria-describedby={describedBy} onChange={(event) => onChange(event.target.checked)} />
    <span className="toggle" aria-hidden="true" />
  </label>;
}

// `section` names the group for the page strip and Find; `setting` marks it as
// the home of one indexed setting, so a jump can land on it.
export function SettingGroup({ title, description, children, id, sectionRef, focusable = false, section, setting }: { title: string; description: string; children: ReactNode; id?: string; sectionRef?: Ref<HTMLElement>; focusable?: boolean; section?: string; setting?: SettingId }) {
  const headingId = id ? `${id}-heading` : undefined;
  return <section className="setting-group" id={id} ref={sectionRef} tabIndex={focusable ? -1 : undefined} aria-labelledby={headingId} data-section={section} data-setting={setting}><header><h2 id={headingId}>{title}</h2><p>{description}</p></header><div className="setting-controls">{children}</div></section>;
}

// `note` renders beside the fieldset rather than inside it, by construction: a
// disclosure inside a disabled fieldset would inherit `disabled` and become
// unreachable. The fieldset keeps its programmatic link to the note through
// aria-describedby, and the wrapper keeps the note visually attached to its
// own group instead of floating a grid gap away from it.
export function OptionGroup<T extends string | number>({ legend, options, value, onChange, disabled, columns, note, setting }: {
  legend: string;
  options: ReadonlyArray<{ value: T; label: ReactNode }>;
  value: T;
  onChange: (next: T) => void;
  disabled: boolean;
  columns?: "three" | "four" | "five";
  note?: NoteProps;
  setting?: SettingId;
}) {
  const noteId = useId();
  const group = <fieldset disabled={disabled} aria-describedby={note ? noteId : undefined} data-setting={setting}><legend>{legend}</legend><div className={columns ? `segmented compact ${columns}` : "segmented compact"}>
    {options.map((option) => <Button type="button" key={option.value} aria-pressed={value === option.value} onClick={() => onChange(option.value)}>{option.label}</Button>)}
  </div></fieldset>;
  return note ? <div className="option-with-note">{group}<SettingNote id={noteId} {...note} /></div> : group;
}

export const overlayVisibilityOptions = [
  { value: "onInput", label: "On input" },
  { value: "whileControlling", label: "While controlling" },
] as const;

export const overlaySizeOptions = (["small", "medium", "large"] as const)
  .map((value) => ({ value, label: value[0].toUpperCase() + value.slice(1) }));

export type SettingsUpdate = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void;

export function DisclosureButton({ label, expanded, onToggle, controls }: { label: string; expanded: boolean; onToggle: () => void; controls?: string }) {
  return <Button type="button" className="disclosure" aria-expanded={expanded} aria-controls={controls} onClick={onToggle}>{label}</Button>;
}

// Reveals content that is unmounted while collapsed, so the button only claims
// aria-controls while the target exists.
export function Disclosure({ label, expanded, onToggle, controls, children }: { label: string; expanded: boolean; onToggle: () => void; controls?: string; children: ReactNode }) {
  return <>
    <DisclosureButton label={label} expanded={expanded} onToggle={onToggle} controls={expanded ? controls : undefined} />
    {expanded && children}
  </>;
}

// A one-line summary, optionally with further detail behind a disclosure. The
// detail must only add to the summary, never restate it, so nothing is read
// twice when it is open. `about` names the setting in the button label:
// several notes can share a panel, and an unqualified "More about this" would
// leave a screen-reader or switch-access user with identically named controls
// in their list.
export type NoteProps = { summary: string; about?: string; detail?: ReactNode };

// A time set a step at a time with − and +, for settings that take any value
// in a range. `zeroLabel` names a time of 0, such as "Off".
export function TimeStepper({ label, value, range, onChange, disabled = false, zeroLabel, note, setting }: {
  label: string;
  value: number;
  range: TimeRange;
  onChange: (next: number) => void;
  disabled?: boolean;
  zeroLabel?: string;
  note?: NoteProps;
  setting?: SettingId;
}) {
  const noteId = useId();
  const step = Number((range.step / 1000).toFixed(3));
  // A fieldset, like OptionGroup: the legend names the group, and disabling
  // it disables both buttons.
  const stepper = <fieldset className="exact-speed time-stepper" disabled={disabled} data-setting={setting} aria-describedby={note ? noteId : undefined}>
    <legend>{label}</legend>
    <div className="scan-rate-stepper">
      <Button type="button" aria-label={`Decrease ${label.toLowerCase()} by ${step} seconds`} disabled={value <= range.min} onClick={() => onChange(stepTime(value, -1, range))}>−</Button>
      <output aria-label={label} aria-live="polite">{value === 0 && zeroLabel ? zeroLabel : formatSeconds(value)}</output>
      <Button type="button" aria-label={`Increase ${label.toLowerCase()} by ${step} seconds`} disabled={value >= range.max} onClick={() => onChange(stepTime(value, 1, range))}>+</Button>
    </div>
  </fieldset>;
  return note ? <div className="option-with-note">{stepper}<SettingNote id={noteId} {...note} /></div> : stepper;
}

export function SettingNote({ id, summary, about, detail }: NoteProps & { id?: string }) {
  const [expanded, setExpanded] = useState(false);
  const generatedId = useId();
  const summaryId = id ?? generatedId;
  const detailId = `${summaryId}-detail`;
  return <div className="setting-note-block">
    <p className="setting-note" id={summaryId}>{summary}</p>
    {about && detail && <Disclosure
      label={expanded ? `Show less about ${about}` : `More about ${about}`}
      expanded={expanded}
      onToggle={() => setExpanded(!expanded)}
      controls={detailId}
    >
      <p className="setting-note" id={detailId}>{detail}</p>
    </Disclosure>}
  </div>;
}
