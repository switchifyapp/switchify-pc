import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export const actions = {
  select: "Select",
  next: "Next",
  back: "Previous",
  reverse: "Reverse direction",
  stop: "Stop scanning",
  pause: "Pause / resume",
} as const;
export type SwitchAction = keyof typeof actions;
export type Binding = {
  id: string;
  name: string;
  key: string;
  pressAction: SwitchAction;
  holdActions: SwitchAction[];
};
export type SwitchSettings = {
  schemaVersion: number;
  holdIntervalMs: number;
  bindings: Binding[];
};
export type SwitchState = {
  settings: SwitchSettings;
  capture: { active: boolean; key: string | null; error: string | null };
  supported: boolean;
  error: string | null;
  escapeHoldMs: number;
  unavailableKeys: string[];
};
const defaults: SwitchSettings = {
  schemaVersion: 1,
  holdIntervalMs: 1000,
  bindings: [],
};
export function useSwitches() {
  const [state, setState] = useState<SwitchState | null>(null);
  const [settings, setSettings] = useState(defaults);
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [capturing, setCapturing] = useState(false);
  const model = useRef({
    settings: defaults,
    revision: 0,
    saved: 0,
    pending: 0,
    locked: false,
    state: null as SwitchState | null,
  });
  const captureRequest = useRef(0);
  const queue = useRef(Promise.resolve());
  const runtime = useRef(0);
  const receive = (next: SwitchState) => {
    model.current.state = next;
    setState(next);
    if (
      model.current.pending === 0 &&
      model.current.revision === model.current.saved
    ) {
      model.current.settings = next.settings;
      setSettings(next.settings);
    }
  };
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    if (!("__TAURI_INTERNALS__" in window)) {
      receive({
        settings: defaults,
        capture: { active: false, key: null, error: null },
        supported: false,
        error: null,
        escapeHoldMs: 4000,
        unavailableKeys: [],
      });
      return;
    }
    void listen<SwitchState>("switches-changed", (e) => {
      runtime.current++;
      if (alive) receive(e.payload);
    })
      .then((unlisten) => {
        if (alive) stop = unlisten;
        else unlisten();
        const stamp = runtime.current;
        return invoke<SwitchState>("get_switches").then((next) => {
          if (alive && stamp === runtime.current) receive(next);
        });
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
      stop?.();
    };
  }, []);
  const save = (next: SwitchSettings, revision: number) => {
    const m = model.current;
    m.pending++;
    setPending(m.pending);
    queue.current = queue.current
      .then(async () => {
        if (revision !== m.revision) return;
        setError(null);
        const stamp = runtime.current;
        const result = await invoke<SwitchState>("save_switches", {
          settings: next,
        });
        m.saved = revision;
        if (stamp === runtime.current) receive(result);
      })
      .catch((e) => setError(String(e)))
      .finally(() => {
        m.pending--;
        setPending(m.pending);
      });
  };
  const update = (next: SwitchSettings) => {
    const m = model.current;
    if (m.locked || m.state?.capture.active || !m.state?.supported) return;
    m.settings = next;
    m.revision++;
    setSettings(next);
    save(next, m.revision);
  };
  const flush = async () => {
    await queue.current;
    if (model.current.revision !== model.current.saved)
      throw new Error(
        "Save switch assignments before enabling scanning. Use Retry save.",
      );
    if (model.current.state?.capture.active)
      throw new Error("Finish learning the switch before enabling scanning.");
  };
  const capture = async () => {
    if (model.current.locked) return;
    const request = ++captureRequest.current;
    setCapturing(true);
    try {
      await flush();
      if (request !== captureRequest.current) return;
      setError(null);
      if (model.current.state)
        receive({
          ...model.current.state,
          capture: { active: true, key: null, error: null },
        });
      const stamp = runtime.current;
      const result = await invoke<SwitchState>("begin_switch_capture");
      if (request === captureRequest.current && stamp === runtime.current)
        receive(result);
    } catch (e) {
      setError(String(e));
      if (model.current.state)
        receive({
          ...model.current.state,
          capture: { active: false, key: null, error: String(e) },
        });
    } finally {
      setCapturing(false);
    }
  };
  const cancelCapture = async () => {
    captureRequest.current++;
    try {
      const stamp = runtime.current;
      const result = await invoke<SwitchState>("cancel_switch_capture");
      if (stamp === runtime.current) receive(result);
    } catch (e) {
      setError(String(e));
    }
  };
  return {
    state,
    settings,
    pending,
    error,
    capturing,
    update,
    flush,
    capture,
    cancelCapture,
    retry: () => save(model.current.settings, model.current.revision),
    unsaved: model.current.revision !== model.current.saved,
    setLocked: (value: boolean) => {
      model.current.locked = value;
    },
  };
}
export type SwitchController = ReturnType<typeof useSwitches>;
