import { SwitchPractice } from "../scanning/SwitchPractice";
import { createPortal } from "react-dom";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { ChevronDown, ChevronUp, Keyboard, Smartphone, Trash2, X } from "lucide-react";
import { useRemoteSwitches, type RemoteSlot } from "../scanning/useRemoteSwitches";
import {
  actions,
  type Binding,
  type SwitchAction,
  type SwitchController,
} from "../scanning/useSwitches";
import {
  Disclosure,
  OptionGroup,
  SettingGroup,
  SettingNote,
  secondsOptions,
} from "./controls";

const holdIntervalPresets = [500, 1000, 1500, 2000, 3000] as const;
const holdIntervalValues = [250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000];
const newId = "new";

function seconds(ms: number) {
  const value = ms / 1000;
  return `${Number.isInteger(value) ? value : value.toFixed(2).replace(/0$/, "")}s`;
}

// The emergency exit must outlast the longest hold list, so it grows with the
// interval. Mirrors the backend rule so the shown number matches behaviour.
export function escapeHoldMs(bindings: Binding[], holdIntervalMs: number) {
  const longest = Math.max(0, ...bindings.map((b) => b.holdActions.length));
  return longest ? Math.max(4000, (longest + 2) * holdIntervalMs) : 4000;
}

function summary(binding: Binding) {
  const hold = binding.holdActions.length
    ? `Hold: ${binding.holdActions.map((a) => actions[a]).join(", ")}`
    : "No hold actions";
  return `${actions[binding.pressAction]} · ${hold}`;
}

function ActionSelect({
  label,
  value,
  onChange,
}: {
  label: string;
  value: SwitchAction;
  onChange: (value: SwitchAction) => void;
}) {
  return (
    <select
      aria-label={label}
      value={value}
      onChange={(e) => onChange(e.target.value as SwitchAction)}
    >
      {Object.entries(actions).map(([value, label]) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </select>
  );
}

function KeyBadge({ value, unavailable }: { value: string; unavailable: boolean }) {
  return value ? (
    <kbd className="key-badge" data-unavailable={unavailable || undefined}>
      {value}
      {unavailable && <span className="sr-only"> (unavailable on this computer)</span>}
    </kbd>
  ) : (
    <span className="key-badge key-badge-empty">No key yet</span>
  );
}

function SwitchEditor({
  id,
  binding,
  isNew,
  disabled,
  holdIntervalMs,
  unavailable,
  keyError,
  onChange,
  onLearn,
  onDone,
  onRemove,
  nameRef,
  remote,
  keyboardEntry,
  entryPending,
  onKeyboardEntry,
}: {
  keyboardEntry: boolean;
  entryPending: boolean;
  onKeyboardEntry: (active: boolean) => void;
  id?: string;
  binding: Binding;
  isNew: boolean;
  disabled: boolean;
  holdIntervalMs: number;
  unavailable: boolean;
  keyError: string | null;
  onChange: (next: Binding) => void;
  onLearn: () => void;
  onDone: () => void;
  onRemove: () => void;
  nameRef: React.RefObject<HTMLInputElement | null>;
  /** Present for a switch forwarded from Switchify Remote: its slot and the
   *  slots it could move to. Remote switches have a default name, so a new
   *  one can be saved unnamed. */
  remote?: { slot: number; free: number[]; onSlot: (slot: number) => void };
}) {
  const name = binding.name || (remote ? `Remote switch ${remote.slot + 1}` : "this switch");
  const keyErrorId = useId();
  const canSave = !isNew || (!!binding.key && (!!binding.name.trim() || !!remote));
  const move = (index: number, delta: number) => {
    const holdActions = [...binding.holdActions];
    [holdActions[index], holdActions[index + delta]] = [
      holdActions[index + delta],
      holdActions[index],
    ];
    onChange({ ...binding, holdActions });
  };
  return (
    <fieldset id={id} className="switch-editor" disabled={disabled}>
      <label className="field">
        <span>Name</span>
        <input
          ref={nameRef}
          aria-label={isNew ? "New switch name" : `Name for ${binding.key}`}
          value={binding.name}
          maxLength={64}
          placeholder="Head switch, foot pedal, sip..."
          onChange={(e) => onChange({ ...binding, name: e.target.value })}
        />
      </label>
      <div className="field">
        <p className="setting-note">
          Switches on this computer can appear as keys to other apps. To type a
          name with the computer keyboard, choose Type with keyboard. Switch
          control pauses until you resume or close this editor. You can use Tab
          and Enter to reach and activate Resume switch control.
        </p>
        <p className="setting-note">If you rely on switches, keep switch control on and use the scanning keyboard from the point action menu to enter the name.</p>
        <button type="button" className="secondary" disabled={entryPending}
          aria-pressed={keyboardEntry} onClick={() => onKeyboardEntry(!keyboardEntry)}>
          {keyboardEntry ? "Resume switch control" : "Type with keyboard"}
        </button>
        {keyboardEntry && <p role="status">Keyboard typing is on. Assigned keys type normally; switch scanning is paused.</p>}
      </div>
      {remote ? (
        <label className="field">
          <span>Remote switch</span>
          <select
            aria-label={`Remote switch number for ${name}`}
            value={remote.slot}
            onChange={(e) => remote.onSlot(Number(e.target.value))}
          >
            {[remote.slot, ...remote.free]
              .sort((a, b) => a - b)
              .map((slot) => (
                <option key={slot} value={slot}>
                  Switch {slot + 1} on the Forwarding screen
                </option>
              ))}
          </select>
        </label>
      ) : (
        <div className="field">
          <span>Key</span>
          <div className="key-row">
            <KeyBadge value={binding.key} unavailable={unavailable} />
            <button
              type="button"
              className="secondary"
              aria-label={isNew ? "Learn switch key" : `Learn another key for ${name}`}
              aria-describedby={keyError || unavailable ? keyErrorId : undefined}
              disabled={keyboardEntry || entryPending}
              onClick={onLearn}
            >
              {binding.key ? "Change key" : "Learn key"}
            </button>
          </div>
          {(keyError || unavailable) && (
            <span className="field-error" id={keyErrorId} role="alert">
              {keyError ??
                "This key is unavailable on this computer. Learn another key."}
            </span>
          )}
        </div>
      )}
      <label className="field">
        <span>Press and release</span>
        <ActionSelect
          label={isNew ? "New switch action" : `Normal action for ${name}`}
          value={binding.pressAction}
          onChange={(pressAction) => onChange({ ...binding, pressAction })}
        />
      </label>
      <div className="field hold-actions">
        <span>Hold</span>
        {binding.holdActions.length > 0 && (
          <ol className="hold-list">
            {binding.holdActions.map((action, index) => (
              <li key={index}>
                <span className="hold-step" aria-hidden="true">
                  {index + 1}
                </span>
                <ActionSelect
                  label={`Hold action ${index + 1} for ${name}`}
                  value={action}
                  onChange={(value) =>
                    onChange({
                      ...binding,
                      holdActions: binding.holdActions.map((a, i) =>
                        i === index ? value : a,
                      ),
                    })
                  }
                />
                <button
                  type="button"
                  className="icon-button"
                  disabled={index === 0}
                  aria-label={`Move hold action ${index + 1} up`}
                  onClick={() => move(index, -1)}
                >
                  <ChevronUp size={18} />
                </button>
                <button
                  type="button"
                  className="icon-button"
                  disabled={index === binding.holdActions.length - 1}
                  aria-label={`Move hold action ${index + 1} down`}
                  onClick={() => move(index, 1)}
                >
                  <ChevronDown size={18} />
                </button>
                <button
                  type="button"
                  className="icon-button danger-icon"
                  aria-label={`Remove hold action ${index + 1}`}
                  onClick={() =>
                    onChange({
                      ...binding,
                      holdActions: binding.holdActions.filter(
                        (_, i) => i !== index,
                      ),
                    })
                  }
                >
                  <X size={18} />
                </button>
              </li>
            ))}
          </ol>
        )}
        <p className="setting-note">
          {binding.holdActions.length
            ? `Hold ${binding.holdActions
                .map((a, i) => `${seconds(holdIntervalMs * (i + 1))} for ${actions[a]}`)
                .join(", ")}. Release to run the action shown.`
            : "Holding only freezes movement. Add actions to offer them one by one while held."}
        </p>
        <button
          type="button"
          className="disclosure"
          disabled={binding.holdActions.length >= 32}
          aria-label={`Add hold action for ${name}`}
          onClick={() =>
            onChange({ ...binding, holdActions: [...binding.holdActions, "next"] })
          }
        >
          Add hold action
        </button>
      </div>
      <div className="switch-editor-footer">
        <button
          type="button"
          className={isNew ? "primary" : "secondary"}
          disabled={!canSave}
          onClick={onDone}
        >
          {isNew ? "Save switch" : "Done"}
        </button>
        <button
          type="button"
          className="secondary danger"
          aria-label={isNew ? "Cancel new switch" : `Remove ${name}`}
          onClick={onRemove}
        >
          {isNew ? "Cancel" : "Remove"}
        </button>
      </div>
    </fieldset>
  );
}

// Keep focus off actionable controls throughout learning, including pre-held
// releases and keys outside the native supported set. Losing main-window focus
// cancels learning; Escape is handled by native capture.
function CaptureDialog({ name, onCancel }: { name: string; onCancel: () => void }) {
  const ref = useRef<HTMLElement>(null);
  const titleId = useId();
  const bodyId = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    const swallow = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };
    const focusBack = () => {
      if (!ref.current?.contains(document.activeElement)) ref.current?.focus();
    };
    for (const type of ["keydown", "keyup", "keypress"] as const)
      document.addEventListener(type, swallow, true);
    document.addEventListener("focusin", focusBack);
    return () => {
      for (const type of ["keydown", "keyup", "keypress"] as const)
        document.removeEventListener(type, swallow, true);
      document.removeEventListener("focusin", focusBack);
      previous?.focus();
    };
  }, []);
  return (
    <div className="modal-backdrop">
      <section
        ref={ref}
        className="capture-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={bodyId}
        tabIndex={-1}
      >
        <Keyboard size={40} aria-hidden="true" />
        <h2 id={titleId}>Press and release your switch</h2>
        <p id={bodyId}>
          Learning your switch for {name}. Nothing else responds until a switch
          press is learned. Press Escape to cancel, or click Cancel capture with
          the mouse.
        </p>
        <button type="button" className="secondary" tabIndex={-1} onClick={onCancel}>
          Cancel capture
        </button>
      </section>
    </div>
  );
}

function SwitchConfirmation({ title, action, busy, error, cancel, confirm }: { title: string; action: string; busy: boolean; error: string | null; cancel: () => void; confirm: () => void }) {
  const ref = useRef<HTMLElement>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  useLayoutEffect(() => {
    const background = [...document.body.children].filter((element) => !element.contains(ref.current));
    const previous = background.map((element) => element.hasAttribute("inert"));
    background.forEach((element) => element.setAttribute("inert", ""));
    keepRef.current?.focus();
    return () => background.forEach((element, index) => { if (!previous[index]) element.removeAttribute("inert"); });
  }, []);
  return createPortal(<div className="modal-backdrop"><section ref={ref} className="profile-dialog confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1} onKeyDown={(event) => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); if (!busy) cancel(); }
    if (event.key === "Tab") {
      const buttons = [...(ref.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
      const first = buttons[0]; const last = buttons.at(-1);
      if (!first) { event.preventDefault(); return; }
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
  }}>
    <header><h2 id={titleId}>{title}</h2></header>
    {error && <p role="alert">{error} Your switch is still available. Try again or keep it.</p>}
    <footer><span /><button ref={keepRef} className="secondary" disabled={busy} onClick={cancel}>Keep switch</button><button className="primary danger" disabled={busy} onClick={confirm}>{busy ? "Saving…" : action}</button></footer>
  </section></div>, document.body);
}

export function SwitchesSection({ controller, onDraftChange, suspended = false, mobileConnected }: { controller: SwitchController; onDraftChange?: (draft: boolean) => void; suspended?: boolean; mobileConnected?: boolean }) {
  const { settings, state, pending, unsaved } = controller;
  // A refused capture sets both the general error and the capture error; the
  // key field already shows the latter, so the band only carries save errors.
  const error =
    controller.error && controller.error !== state?.capture.error
      ? controller.error
      : null;
  const [draft, setDraft] = useState<Binding | null>(null);
  // A draft with a slot is a remote switch; it never learns a key.
  const [confirmation, setConfirmation] = useState<{ kind: "local" | "remote" | "draft"; id: string; index?: number; name: string } | null>(null);
  const [confirmBusy, setConfirmBusy] = useState(false);
  const [confirmError, setConfirmError] = useState<string | null>(null);
  const opener = useRef<HTMLElement | null>(null);
  const initialDraft = useRef<Binding | null>(null);
  const requestRemoval = (kind: "local" | "remote" | "draft", binding: Binding, index?: number) => {
    opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setConfirmError(null);
    setConfirmation({ kind, id: binding.id, index, name: binding.name || (kind === "remote" ? `Remote switch ${(index ?? 0) + 1}` : "this switch") });
  };
  const [draftSlot, setDraftSlot] = useState<number | null>(null);
  useEffect(() => { onDraftChange?.(draft !== null); }, [draft, onDraftChange]);
  const remote = useRemoteSwitches();
  const slots = remote.config?.slots ?? [];
  const remoteRows = slots
    .map((slot, index) => ({ slot, index }))
    .filter(({ slot }) => slot.pressAction !== null);
  const freeSlots = slots
    .map((slot, index) => (slot.pressAction === null ? index : -1))
    .filter((index) => index >= 0);
  const remoteId = (index: number) => `remote-${index + 1}`;
  const remoteBinding = (slot: RemoteSlot, index: number): Binding => ({
    id: remoteId(index),
    name: slot.name ?? "",
    key: `Remote ${index + 1}`,
    pressAction: slot.pressAction ?? "select",
    holdActions: slot.holdActions,
  });
  const setSlot = (index: number, slot: RemoteSlot | null) =>
    remote.update(
      slots.map((s, i) =>
        i === index ? (slot ?? { pressAction: null, holdActions: [] }) : s,
      ),
    );
  const slotFrom = (binding: Binding): RemoteSlot => ({
    pressAction: binding.pressAction,
    holdActions: binding.holdActions,
    ...(binding.name.trim() ? { name: binding.name.trim() } : {}),
  });
  const moveSlot = (from: number, to: number) => {
    if (from === to || slots[to]?.pressAction !== null) return;
    remote.update(
      slots.map((s, i) =>
        i === to ? slots[from] : i === from ? { pressAction: null, holdActions: [] } : s,
      ),
    );
    // The row is keyed by its number, so it remounts; keep focus on it.
    focusAfter.current = remoteId(to);
    setExpanded(remoteId(to));
  };
  const [expanded, setExpanded] = useState<string | null>(null);
  const entryOwner = useRef(false);
  const setEntry = useRef(controller.setKeyboardEntry);
  setEntry.current = controller.setKeyboardEntry;
  const [entryPending, setEntryPending] = useState(false);
  const releaseEntry = () => {
    if (!entryOwner.current) return;
    entryOwner.current = false;
    void setEntry.current(false);
  };
  useEffect(() => () => {
    if (entryOwner.current) {
      entryOwner.current = false;
      void setEntry.current(false);
    }
  }, []);
  useEffect(() => {
    if (expanded === null || suspended) releaseEntry();
  }, [expanded, suspended]);
  const toggleEntry = async (active: boolean) => {
    // Mark ownership before awaiting: navigation must queue a release even if
    // the entry command is still in flight.
    entryOwner.current = active;
    setEntryPending(true);
    const changed = await controller.setKeyboardEntry(active);
    setEntryPending(false);
    if (active && changed && entryOwner.current) nameRef.current?.focus();
  };
  const entryProps = {
    keyboardEntry: !!state?.keyboardEntry,
    entryPending,
    onKeyboardEntry: (active: boolean) => { void toggleEntry(active); },
  };
  const [target, setTarget] = useState<string | null>(null);
  // A key error belongs to the row that was learning when it happened, not to
  // whichever row is open later; the backend keeps its last capture error
  // until the next capture begins, so it is consumed once here.
  const [rowError, setRowError] = useState<{ id: string; message: string } | null>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const focusName = useRef(false);
  // Collapsing unmounts the button that was clicked, which would drop focus to
  // the page. Remember where focus should land and move it after the render.
  const editRefs = useRef(new Map<string, HTMLButtonElement>());
  const addRef = useRef<HTMLButtonElement>(null);
  const addRemoteRef = useRef<HTMLButtonElement>(null);
  const focusAfter = useRef<string | null>(null);
  // Consume the handoff only after its DOM commit. A pending passive effect
  // from the previous render can otherwise focus an Add button just before it
  // unmounts, consuming the handoff and leaving focus on the document body.
  useLayoutEffect(() => {
    if (!focusAfter.current) return;
    const id = focusAfter.current;
    focusAfter.current = null;
    // Fall through past controls that are unmounted or disabled: while a new
    // switch is being drafted the Add buttons are not rendered, so focus lands
    // on the draft's name field instead.
    const target = id === newId ? null : editRefs.current.get(id);
    const enabled = (el: HTMLElement | null) => (el && !(el as HTMLButtonElement).disabled ? el : null);
    (target ?? enabled(addRef.current) ?? enabled(addRemoteRef.current) ?? nameRef.current)?.focus();
  });
  const cancel = useRef(controller.cancelCapture);
  cancel.current = controller.cancelCapture;
  useEffect(
    () => () => {
      void cancel.current();
    },
    [],
  );
  useEffect(() => {
    if (
      controller.capturing ||
      !target ||
      !state?.capture.key ||
      state.capture.active
    )
      return;
    const key = state.capture.key;
    if (settings.bindings.some((b) => b.key === key && b.id !== target)) {
      setRowError({ id: target, message: "That key already belongs to another switch." });
      setTarget(null);
      return;
    }
    if (target === newId) {
      setDraft((d) => (d ? { ...d, key } : d));
      focusName.current = true;
    } else
      controller.update({
        ...settings,
        bindings: settings.bindings.map((b) =>
          b.id === target ? { ...b, key } : b,
        ),
      });
    setTarget(null);
  }, [state?.capture.key, state?.capture.active, target, settings, controller]);
  const capturing = controller.capturing || !!state?.capture.active;
  // A capture that ends without a key, whether refused or cancelled by focus
  // loss, must release its target; otherwise the backend's remembered key from
  // an earlier capture could be assigned to this row by a later view.
  useEffect(() => {
    if (capturing || !target || !state || state.capture.key) return;
    if (state.capture.error) setRowError({ id: target, message: state.capture.error });
    setTarget(null);
  }, [capturing, target, state]);
  const disabled = !state?.supported || !!state.error || capturing;
  // A learned key arrives while the editor is disabled; focus the name once the
  // fieldset is enabled again so a new switch can be named straight away.
  useEffect(() => {
    if (!disabled && focusName.current) {
      focusName.current = false;
      nameRef.current?.focus();
    }
  }, [disabled]);
  const edit = (binding: Binding) =>
    controller.update({
      ...settings,
      bindings: settings.bindings.map((b) =>
        b.id === binding.id ? binding : b,
      ),
    });
  const learn = (id: string) => {
    releaseEntry();
    setRowError(null);
    setTarget(id);
    void controller.capture();
  };
  const startAdd = () => {
    setDraftSlot(null);
    initialDraft.current = { id: newId, name: "", key: "", pressAction: "select", holdActions: [] };
    setDraft(initialDraft.current);
    setExpanded(newId);
    learn(newId);
  };
  const startAddRemote = () => {
    const slot = freeSlots[0];
    if (slot === undefined) return;
    setDraftSlot(slot);
    initialDraft.current = { id: newId, name: "", key: `Remote ${slot + 1}`, pressAction: "select", holdActions: [] };
    setDraft(initialDraft.current);
    setExpanded(newId);
    // The Add buttons unmount while drafting; the name field takes focus.
    focusAfter.current = newId;
  };
  const cancelAdd = () => {
    setDraft(null);
    setDraftSlot(null);
    setExpanded(null);
    setRowError(null);
    focusAfter.current = newId;
    if (target === newId) {
      setTarget(null);
      void controller.cancelCapture();
    }
  };
  const cancelConfirmation = () => {
    setConfirmation(null);
    // The portal restores the background before focus returns to its opener.
    requestAnimationFrame(() => { if (opener.current?.isConnected) opener.current.focus(); });
  };
  const confirmRemoval = async () => {
    if (!confirmation || confirmBusy) return;
    if (confirmation.kind === "draft") { setConfirmation(null); cancelAdd(); return; }
    setConfirmBusy(true);
    setConfirmError(null);
    const removed = confirmation.kind === "local" ? await controller.remove(confirmation.id) : await remote.remove(confirmation.index!);
    setConfirmBusy(false);
    if (removed) {
      releaseEntry();
      if (expanded === confirmation.id) setExpanded(null);
      setConfirmation(null);
      focusAfter.current = newId;
    }
    else setConfirmError("Could not remove the switch.");
  };
  const add = () => {
    if (!draft) return;
    if (draftSlot !== null) {
      setSlot(draftSlot, slotFrom(draft));
      setDraft(null);
      setDraftSlot(null);
      setExpanded(null);
      focusAfter.current = newId;
      return;
    }
    if (!draft.name.trim() || !draft.key) return;
    controller.update({
      ...settings,
      bindings: [
        ...settings.bindings,
        { ...draft, id: crypto.randomUUID(), name: draft.name.trim() },
      ],
    });
    setDraft(null);
    setExpanded(null);
    focusAfter.current = newId;
  };
  const escapeMs = escapeHoldMs(
    [...settings.bindings, ...remoteRows.map(({ slot, index }) => remoteBinding(slot, index))],
    settings.holdIntervalMs,
  );
  const isPreset = (holdIntervalPresets as readonly number[]).includes(
    settings.holdIntervalMs,
  );
  const [showExact, setShowExact] = useState(!isPreset);
  useEffect(() => {
    if (!isPreset) setShowExact(true);
  }, [isPreset]);
  const exactId = useId();
  const listId = useId();
  const captureName =
    target === newId
      ? "the new switch"
      : (settings.bindings.find((b) => b.id === target)?.name ?? "this switch");
  const errorFor = (id: string) => (rowError?.id === id ? rowError.message : null);
  return (
    <>
      {state?.keyboardEntry && !expanded && <div role="status" className="setting-note">
        Keyboard typing is on. Switch scanning is paused.
        <button type="button" className="secondary" disabled={entryPending} onClick={() => { void toggleEntry(false); }}>Resume switch control</button>
      </div>}
      {confirmation && !suspended && <SwitchConfirmation title={confirmation.kind === "draft" ? "Discard this new switch?" : `Remove ${confirmation.name}?`} action={confirmation.kind === "draft" ? "Discard switch" : "Remove switch"} busy={confirmBusy} error={confirmError} cancel={cancelConfirmation} confirm={() => void confirmRemoval()} />}
      {capturing && target && !suspended && (
        <CaptureDialog
          name={captureName}
          onCancel={() => {
            setTarget(null);
            void controller.cancelCapture();
          }}
        />
      )}
      <SettingGroup
        title="Switches"
        description="Add switches on this computer or switches forwarded from Switchify Remote, and choose what each one does."
      >
        <SwitchPractice disabled={!!state?.keyboardEntry || entryPending || capturing || !!pending || unsaved || !!remote.pending || remote.unsaved || !!draft || suspended} />
        <div className="switch-source-summary">
          <p className="setting-note">{!state ? "Loading local switch configuration..." : settings.bindings.length === 0
            ? "No local switches configured. Use Add switch to learn a switch."
            : `${settings.bindings.length} local ${settings.bindings.length === 1 ? "switch configured" : "switches configured"}.`}</p>
          <p className="setting-note">Remote assignments are numbered forwarding slots in Switchify Remote, not detected physical switches. The supplied presets can be changed or removed.</p>
          <p className="setting-note" role="status">{mobileConnected === true
            ? "Mobile device connected. This does not confirm that remote assignments match your switches."
            : mobileConnected === false
              ? "No mobile device connected. Remote assignments are available for a future connection."
              : "Mobile connection status is unavailable here."} A connection alone does not verify a physical switch press.</p>
        </div>
        <SettingNote
          about="switches"
          summary="Press and release a switch to run its action. Hold it to step through its hold actions instead."
          detail={`Movement freezes while a switch is held, and each hold action is offered on screen in turn. Escape resets the scan. Holding any switch for ${seconds(escapeMs)} also resets it. Remote switches are numbered as they appear on the Forwarding screen in Switchify Remote and share the hold timing below.`}
        />
        <p className="setting-note switch-status" role="status">
          {pending || remote.pending
            ? "Saving switches..."
            : unsaved || remote.unsaved
              ? "Switch assignments have unsaved changes."
              : "Changes save automatically and apply straight away."}
        </p>
        {remote.error && (
          <div className="dialog-error switch-error" role="alert">
            <span>{remote.error}</span>
            <button
              type="button"
              className="secondary"
              disabled={!!remote.pending}
              onClick={remote.retry}
            >
              Retry save
            </button>
          </div>
        )}
        {(error || state?.error) && (
          <div className="dialog-error switch-error" role="alert">
            <span>{error || state?.error}</span>
            {error && unsaved && (
              <button
                type="button"
                className="secondary"
                disabled={disabled || !!pending}
                onClick={controller.retry}
              >
                Retry save
              </button>
            )}
          </div>
        )}
        {!settings.bindings.length && !remoteRows.length && !draft ? (
          <div className="empty-state switch-empty">
            <Keyboard size={28} aria-hidden="true" />
            <h3>No switches yet</h3>
            <p>
              Automatic scanning needs a Select switch. Manual scanning also
              needs Next and Previous.
            </p>
            <div className="switch-list-actions">
              <button
                type="button"
                className="primary"
                ref={addRef}
                disabled={disabled}
                onClick={startAdd}
              >
                Add switch
              </button>
              <button
                type="button"
                className="secondary"
                ref={addRemoteRef}
                disabled={!remote.config || !freeSlots.length}
                onClick={startAddRemote}
              >
                Add remote switch
              </button>
            </div>
          </div>
        ) : (
          <div className="switch-list" id={listId}>
            {settings.bindings.map((binding) => {
              const open = expanded === binding.id;
              const name = binding.name || "Unnamed switch";
              const unavailable = !!state?.unavailableKeys.includes(binding.key);
              return (
                <article
                  key={binding.id}
                  className="switch-row"
                  data-open={open || undefined}
                >
                  <div className="switch-row-summary">
                    <Keyboard size={20} aria-hidden="true" />
                    <div>
                      <h3>{name}</h3>
                      <p>On this computer</p>
                      <p>{summary(binding)}</p>
                    </div>
                    <KeyBadge value={binding.key} unavailable={unavailable} />
                    <button
                      type="button"
                      className="secondary"
                      ref={(el) => {
                        if (el) editRefs.current.set(binding.id, el);
                        else editRefs.current.delete(binding.id);
                      }}
                      aria-expanded={open}
                      aria-controls={open ? `${listId}-${binding.id}` : undefined}
                      aria-label={`${open ? "Close" : "Edit"} ${name}`}
                      disabled={disabled || !!draft}
                      onClick={() => {
                        setRowError(null);
                        setExpanded(open ? null : binding.id);
                      }}
                    >
                      {open ? "Close" : "Edit"}
                    </button>
                    <button
                      type="button"
                      className="icon-button danger-icon"
                      aria-label={`Remove ${name}`}
                      disabled={disabled || !!pending || unsaved}
                      onClick={() => requestRemoval("local", binding)}
                    >
                      <Trash2 size={18} />
                    </button>
                  </div>
                  {open && (
                    <SwitchEditor
                      {...entryProps}
                      id={`${listId}-${binding.id}`}
                      binding={binding}
                      isNew={false}
                      disabled={disabled}
                      holdIntervalMs={settings.holdIntervalMs}
                      unavailable={unavailable}
                      keyError={errorFor(binding.id)}
                      onChange={edit}
                      onLearn={() => learn(binding.id)}
                      onDone={() => {
                        focusAfter.current = binding.id;
                        setExpanded(null);
                      }}
                      onRemove={() => requestRemoval("local", binding)}
                      nameRef={nameRef}
                    />
                  )}
                </article>
              );
            })}
            {remoteRows.map(({ slot, index }) => {
              const binding = remoteBinding(slot, index);
              const open = expanded === binding.id;
              const name = binding.name || `Remote switch ${index + 1}`;
              return (
                <article
                  key={binding.id}
                  className="switch-row"
                  data-open={open || undefined}
                >
                  <div className="switch-row-summary">
                    <Smartphone size={20} aria-hidden="true" />
                    <div>
                      <h3>{name}</h3>
                      <p>Remote forwarding slot {index + 1}</p>
                      <p>{summary(binding)}</p>
                    </div>
                    <KeyBadge value={binding.key} unavailable={false} />
                    <button
                      type="button"
                      className="secondary"
                      ref={(el) => {
                        if (el) editRefs.current.set(binding.id, el);
                        else editRefs.current.delete(binding.id);
                      }}
                      aria-expanded={open}
                      aria-controls={open ? `${listId}-${binding.id}` : undefined}
                      aria-label={`${open ? "Close" : "Edit"} ${name}`}
                      disabled={!!draft}
                      onClick={() => setExpanded(open ? null : binding.id)}
                    >
                      {open ? "Close" : "Edit"}
                    </button>
                    <button
                      type="button"
                      className="icon-button danger-icon"
                      aria-label={`Remove ${name}`}
                      disabled={!!remote.pending || remote.unsaved}
                      onClick={() => requestRemoval("remote", binding, index)}
                    >
                      <Trash2 size={18} />
                    </button>
                  </div>
                  {open && (
                    <SwitchEditor
                      {...entryProps}
                      id={`${listId}-${binding.id}`}
                      binding={binding}
                      isNew={false}
                      disabled={false}
                      holdIntervalMs={settings.holdIntervalMs}
                      unavailable={false}
                      keyError={null}
                      onChange={(next) => setSlot(index, slotFrom(next))}
                      onLearn={() => {}}
                      onDone={() => {
                        focusAfter.current = binding.id;
                        setExpanded(null);
                      }}
                      onRemove={() => requestRemoval("remote", binding, index)}
                      nameRef={nameRef}
                      remote={{ slot: index, free: freeSlots, onSlot: (to) => moveSlot(index, to) }}
                    />
                  )}
                </article>
              );
            })}
            {draft && (
              <article className="switch-row" data-open>
                <div className="switch-row-summary">
                  {draftSlot === null ? <Keyboard size={20} aria-hidden="true" /> : <Smartphone size={20} aria-hidden="true" />}
                  <div>
                    <h3>{draftSlot === null ? "New switch" : "New remote switch"}</h3>
                    <p>
                      {draftSlot !== null
                        ? "Choose its number and actions, then save."
                        : draft.key
                          ? "Name it and choose its actions, then save."
                          : "Learn its key first."}
                    </p>
                  </div>
                </div>
                <SwitchEditor
                  {...entryProps}
                  binding={draft}
                  isNew
                  disabled={draftSlot === null && disabled}
                  holdIntervalMs={settings.holdIntervalMs}
                  unavailable={false}
                  keyError={errorFor(newId)}
                  onChange={setDraft}
                  onLearn={() => learn(newId)}
                  onDone={add}
                  onRemove={() => {
                    if (JSON.stringify(draft) === JSON.stringify(initialDraft.current)) cancelAdd();
                    else requestRemoval("draft", draft);
                  }}
                  nameRef={nameRef}
                  remote={
                    draftSlot === null
                      ? undefined
                      : {
                          slot: draftSlot,
                          free: freeSlots.filter((s) => s !== draftSlot),
                          onSlot: (slot) => {
                            setDraftSlot(slot);
                            setDraft({ ...draft, key: `Remote ${slot + 1}` });
                          },
                        }
                  }
                />
              </article>
            )}
            {!draft && (
              <div className="switch-list-actions">
                <button
                  type="button"
                  className="secondary"
                  ref={addRef}
                  disabled={disabled || settings.bindings.length >= 128}
                  onClick={startAdd}
                >
                  Add switch
                </button>
                <button
                  type="button"
                  className="secondary"
                  ref={addRemoteRef}
                  disabled={!remote.config || !freeSlots.length}
                  onClick={startAddRemote}
                >
                  Add remote switch
                </button>
              </div>
            )}
          </div>
        )}
      </SettingGroup>
      <SettingGroup
        title="Hold timing"
        description="How long a switch is held before each hold action is offered."
      >
        <div className="repeat-options">
          <OptionGroup<number>
            legend="Hold action interval"
            columns="five"
            disabled={disabled}
            value={settings.holdIntervalMs}
            onChange={(holdIntervalMs) =>
              controller.update({ ...settings, holdIntervalMs })
            }
            options={secondsOptions(holdIntervalPresets)}
          />
          <Disclosure
            label={showExact ? "Hide exact interval" : "Set an exact interval"}
            expanded={showExact}
            onToggle={() => setShowExact(!showExact)}
            controls={exactId}
          >
            <label className="exact-speed" id={exactId}>
              <span>Exact interval</span>
              <select
                aria-label="Exact hold action interval"
                disabled={disabled}
                value={settings.holdIntervalMs}
                onChange={(e) =>
                  controller.update({
                    ...settings,
                    holdIntervalMs: Number(e.target.value),
                  })
                }
              >
                {holdIntervalValues.map((value) => (
                  <option key={value} value={value}>
                    {seconds(value)}
                  </option>
                ))}
              </select>
            </label>
          </Disclosure>
          <p className="setting-note">
            The first action appears after one interval and the next after
            each further interval. The last action stays offered. Holding any
            switch for {seconds(escapeMs)} resets the scan.
          </p>
        </div>
      </SettingGroup>
    </>
  );
}
