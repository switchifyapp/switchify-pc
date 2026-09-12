import { useEffect, useId, useRef, useState } from "react";
import { ChevronDown, ChevronUp, Keyboard, Trash2, X } from "lucide-react";
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
}: {
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
}) {
  const name = binding.name || "this switch";
  const keyErrorId = useId();
  const canSave = !isNew || (!!binding.name.trim() && !!binding.key);
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
        <span>Key</span>
        <div className="key-row">
          <KeyBadge value={binding.key} unavailable={unavailable} />
          <button
            type="button"
            className="secondary"
            aria-label={isNew ? "Learn switch key" : `Learn another key for ${name}`}
            aria-describedby={keyError || unavailable ? keyErrorId : undefined}
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

// Learning must own the keyboard: on Windows the reserved press never reaches
// the page but its release does, and a Space release on a focused button
// clicks it, while an unreserved key would scroll Settings. A separate window
// is not an option because losing main-window focus cancels the capture. So a
// modal holds focus on itself, not on a control, and swallows every key event
// until learning ends. Escape still reaches the backend through its hotkey.
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
          Learning the key for {name}. Nothing else responds until a key is
          learned. Press Escape to cancel, or click Cancel capture with the
          mouse.
        </p>
        <button type="button" className="secondary" tabIndex={-1} onClick={onCancel}>
          Cancel capture
        </button>
      </section>
    </div>
  );
}

export function SwitchesSection({
  controller,
  locked,
}: {
  controller: SwitchController;
  locked: boolean;
}) {
  const { settings, state, pending, unsaved } = controller;
  // A refused capture sets both the general error and the capture error; the
  // key field already shows the latter, so the band only carries save errors.
  const error =
    controller.error && controller.error !== state?.capture.error
      ? controller.error
      : null;
  const [draft, setDraft] = useState<Binding | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
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
  const focusAfter = useRef<string | null>(null);
  useEffect(() => {
    if (!focusAfter.current) return;
    const id = focusAfter.current;
    focusAfter.current = null;
    // While a new switch is being drafted the Add switch button is not rendered,
    // so a removal elsewhere hands focus to the draft's name field instead.
    const target = id === newId ? null : editRefs.current.get(id);
    (target ?? addRef.current ?? nameRef.current)?.focus();
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
  const disabled = locked || !state?.supported || !!state.error || capturing;
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
  const remove = (id: string) => {
    focusAfter.current = newId;
    controller.update({
      ...settings,
      bindings: settings.bindings.filter((b) => b.id !== id),
    });
  };
  const learn = (id: string) => {
    setRowError(null);
    setTarget(id);
    void controller.capture();
  };
  const startAdd = () => {
    setDraft({ id: newId, name: "", key: "", pressAction: "select", holdActions: [] });
    setExpanded(newId);
    learn(newId);
  };
  const cancelAdd = () => {
    setDraft(null);
    setExpanded(null);
    setRowError(null);
    focusAfter.current = newId;
    if (target === newId) {
      setTarget(null);
      void controller.cancelCapture();
    }
  };
  const add = () => {
    if (!draft?.name.trim() || !draft.key) return;
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
  const escapeMs = escapeHoldMs(settings.bindings, settings.holdIntervalMs);
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
      {capturing && target && (
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
        description="Add keyboard switches and choose what each one does."
      >
        <SettingNote
          about="switches"
          summary="Press and release a switch to run its action. Hold it to step through its hold actions instead."
          detail={`Movement freezes while a switch is held, and each hold action is offered on screen in turn. Escape disables switch control immediately. Holding any assigned switch for ${seconds(escapeMs)} also disables it.`}
        />
        <p className="setting-note switch-status" role="status">
          {pending
            ? "Saving switches..."
            : unsaved
              ? "Switch assignments have unsaved changes."
              : locked
                ? "Disable scanning to change switches."
                : "Changes save automatically."}
        </p>
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
        {!settings.bindings.length && !draft ? (
          <div className="empty-state switch-empty">
            <Keyboard size={28} aria-hidden="true" />
            <h3>No switches yet</h3>
            <p>
              Automatic scanning needs a Select switch. Manual scanning also
              needs Next and Previous.
            </p>
            <button
              type="button"
              className="primary"
              ref={addRef}
              disabled={disabled}
              onClick={startAdd}
            >
              Add switch
            </button>
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
                      disabled={disabled}
                      onClick={() => remove(binding.id)}
                    >
                      <Trash2 size={18} />
                    </button>
                  </div>
                  {open && (
                    <SwitchEditor
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
                      onRemove={() => remove(binding.id)}
                      nameRef={nameRef}
                    />
                  )}
                </article>
              );
            })}
            {draft && (
              <article className="switch-row" data-open>
                <div className="switch-row-summary">
                  <Keyboard size={20} aria-hidden="true" />
                  <div>
                    <h3>New switch</h3>
                    <p>
                      {draft.key
                        ? "Name it and choose its actions, then save."
                        : "Learn its key first."}
                    </p>
                  </div>
                </div>
                <SwitchEditor
                  binding={draft}
                  isNew
                  disabled={disabled}
                  holdIntervalMs={settings.holdIntervalMs}
                  unavailable={false}
                  keyError={errorFor(newId)}
                  onChange={setDraft}
                  onLearn={() => learn(newId)}
                  onDone={add}
                  onRemove={cancelAdd}
                  nameRef={nameRef}
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
            switch for {seconds(escapeMs)} disables switch control.
          </p>
        </div>
      </SettingGroup>
    </>
  );
}
