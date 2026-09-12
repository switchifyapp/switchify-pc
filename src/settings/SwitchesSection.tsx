import { useEffect, useRef, useState } from "react";
import {
  actions,
  type Binding,
  type SwitchAction,
  type SwitchController,
} from "../scanning/useSwitches";
import { SettingGroup, OptionGroup, secondsOptions } from "./controls";
function ActionPicker({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: SwitchAction;
  disabled: boolean;
  onChange: (value: SwitchAction) => void;
}) {
  return (
    <label className="exact-speed">
      <span>{label}</span>
      <select
        aria-label={label}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as SwitchAction)}
      >
        {Object.entries(actions).map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </select>
    </label>
  );
}
export function SwitchesSection({
  controller,
  locked,
}: {
  controller: SwitchController;
  locked: boolean;
}) {
  const { settings, state, pending, error, unsaved } = controller;
  const [draft, setDraft] = useState<Binding | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
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
      setFormError("That key already belongs to another switch.");
      setTarget(null);
      return;
    }
    if (target === "new") setDraft((d) => (d ? { ...d, key } : d));
    else
      controller.update({
        ...settings,
        bindings: settings.bindings.map((b) =>
          b.id === target ? { ...b, key } : b,
        ),
      });
    setTarget(null);
  }, [state?.capture.key, state?.capture.active, target, settings, controller]);
  const disabled =
    locked ||
    !state?.supported ||
    !!state.error ||
    controller.capturing ||
    !!state.capture.active;
  const edit = (binding: Binding) =>
    controller.update({
      ...settings,
      bindings: settings.bindings.map((b) =>
        b.id === binding.id ? binding : b,
      ),
    });
  const learn = (id: string) => {
    setFormError(null);
    setTarget(id);
    void controller.capture();
  };
  const add = () => {
    if (!draft?.name.trim() || !draft.key) {
      setFormError("Enter a name and learn the switch key first.");
      return;
    }
    controller.update({
      ...settings,
      bindings: [
        ...settings.bindings,
        { ...draft, id: crypto.randomUUID(), name: draft.name.trim() },
      ],
    });
    setDraft(null);
  };
  return (
    <>
      <SettingGroup
        title="Switches"
        description="Add keyboard switches and choose what each one does."
      >
        <p>
          Press and release a switch to run its normal action. Holding a switch
          pauses movement and offers its hold actions in order. Release to
          choose the action shown on screen.
        </p>
        <p>
          Escape disables switch control immediately. Holding any assigned
          switch for{" "}
          {(settings.bindings.some((b) => b.holdActions.length)
            ? Math.max(
                4000,
                (Math.max(
                  ...settings.bindings.map((b) => b.holdActions.length),
                ) +
                  2) *
                  settings.holdIntervalMs,
              )
            : 4000) / 1000}{" "}
          seconds also disables it.
        </p>
        <p role="status">
          {pending
            ? "Saving switches..."
            : unsaved
              ? "Switch assignments have unsaved changes."
              : "Switch assignments save automatically."}
        </p>
        {(error || state?.error) && <p role="alert">{error || state?.error}</p>}
        {error && unsaved && (
          <button className="secondary" disabled={disabled || !!pending} onClick={controller.retry}>
            Retry save
          </button>
        )}
        {formError && <p role="alert">{formError}</p>}
        {state?.capture.error && <p role="alert">{state.capture.error}</p>}
        {(controller.capturing || state?.capture.active) && (
          <div role="status">
            <p>
              Press and release the switch you want to learn. Escape cancels.
            </p>
            <button className="secondary"
              onClick={() => {
                setTarget(null);
                void controller.cancelCapture();
              }}
            >
              Cancel capture
            </button>
          </div>
        )}
        {!settings.bindings.length && (
          <p>
            No switches added yet. Add a Select action to use automatic
            scanning; manual scanning also needs Next and Previous.
          </p>
        )}
        {settings.bindings.map((binding) => (
          <fieldset
            key={binding.id}
            disabled={disabled}
            className="switch-assignment"
          >
            <legend>{binding.name || "Unnamed switch"}</legend>
            <label className="exact-speed">
              <span>Switch name</span>
              <input
                aria-label={`Name for ${binding.key}`}
                value={binding.name}
                maxLength={64}
                onChange={(e) => edit({ ...binding, name: e.target.value })}
              />
            </label>
            <p>Key: {binding.key}</p>
            {state?.unavailableKeys.includes(binding.key) && (
              <p role="alert">
                This key is unavailable on this computer. Learn another key.
              </p>
            )}
            <button className="secondary" onClick={() => learn(binding.id)}>
              Learn another key for {binding.name}
            </button>
            <ActionPicker
              label={`Normal action for ${binding.name}`}
              value={binding.pressAction}
              disabled={disabled}
              onChange={(pressAction) => edit({ ...binding, pressAction })}
            />
            <p>Hold actions</p>
            {binding.holdActions.length === 0 && (
              <p>No hold actions assigned.</p>
            )}
            <ol>
              {binding.holdActions.map((action, index) => (
                <li key={index}>
                  <ActionPicker
                    label={`Hold action ${index + 1} for ${binding.name}`}
                    value={action}
                    disabled={disabled}
                    onChange={(value) =>
                      edit({
                        ...binding,
                        holdActions: binding.holdActions.map((a, i) =>
                          i === index ? value : a,
                        ),
                      })
                    }
                  />
                  <button className="secondary"
                    disabled={disabled || index === 0}
                    aria-label={`Move hold action ${index + 1} up`}
                    onClick={() => {
                      const holdActions = [...binding.holdActions];
                      [holdActions[index - 1], holdActions[index]] = [
                        holdActions[index],
                        holdActions[index - 1],
                      ];
                      edit({ ...binding, holdActions });
                    }}
                  >
                    Move up
                  </button>
                  <button className="secondary"
                    disabled={
                      disabled || index === binding.holdActions.length - 1
                    }
                    aria-label={`Move hold action ${index + 1} down`}
                    onClick={() => {
                      const holdActions = [...binding.holdActions];
                      [holdActions[index + 1], holdActions[index]] = [
                        holdActions[index],
                        holdActions[index + 1],
                      ];
                      edit({ ...binding, holdActions });
                    }}
                  >
                    Move down
                  </button>
                  <button className="secondary"
                    aria-label={`Remove hold action ${index + 1}`}
                    onClick={() =>
                      edit({
                        ...binding,
                        holdActions: binding.holdActions.filter(
                          (_, i) => i !== index,
                        ),
                      })
                    }
                  >
                    Remove action
                  </button>
                </li>
              ))}
            </ol>
            <button className="secondary"
              disabled={disabled || binding.holdActions.length >= 32}
              onClick={() =>
                edit({
                  ...binding,
                  holdActions: [...binding.holdActions, "next"],
                })
              }
            >
              Add hold action for {binding.name}
            </button>
            <button className="secondary"
              onClick={() =>
                controller.update({
                  ...settings,
                  bindings: settings.bindings.filter(
                    (b) => b.id !== binding.id,
                  ),
                })
              }
            >
              Remove {binding.name}
            </button>
          </fieldset>
        ))}
        {draft ? (
          <fieldset className="switch-assignment" disabled={disabled}>
            <legend>Add switch</legend>
            <label className="exact-speed">
              <span>New switch name</span>
              <input
                value={draft.name}
                maxLength={64}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
            </label>
            <p>{draft.key ? `Key: ${draft.key}` : "No key learned yet."}</p>
            <button className="secondary" onClick={() => learn("new")}>Learn switch key</button>
            <ActionPicker
              label="New switch action"
              value={draft.pressAction}
              disabled={disabled}
              onChange={(pressAction) => setDraft({ ...draft, pressAction })}
            />
            <button className="secondary" onClick={add}>Save new switch</button>
            <button className="secondary"
              onClick={() => {
                setDraft(null);
                setTarget(null);
              }}
            >
              Cancel new switch
            </button>
          </fieldset>
        ) : (
          <button className="secondary"
            disabled={disabled || settings.bindings.length >= 128}
            onClick={() =>
              setDraft({
                id: "new",
                name: "",
                key: "",
                pressAction: "select",
                holdActions: [],
              })
            }
          >
            Add switch
          </button>
        )}
      </SettingGroup>
      <SettingGroup
        title="Hold timing"
        description="The first action appears after this interval. Each following interval offers the next action; the final action stays selected."
      >
        <OptionGroup<number>
          legend="Hold action interval"
          disabled={disabled}
          value={settings.holdIntervalMs}
          onChange={(holdIntervalMs) =>
            controller.update({ ...settings, holdIntervalMs })
          }
          options={secondsOptions([
            250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000,
          ])}
        />
      </SettingGroup>
    </>
  );
}
