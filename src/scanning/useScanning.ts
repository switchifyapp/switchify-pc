import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { defaultScanPreferences } from "./preferences";
export const scannerColors = ["red", "green", "blue", "yellow"] as const;
export type ScannerColor = typeof scannerColors[number];
export type KeyboardLayout = "qwerty" | "simpleQwerty" | "commonLetters";
export type PointScanConfig = {
  controlMode: "point" | "mouse";
  /** What Select opens when scanning starts afresh. Older saved state may lack it; the runtime then opens Home. */
  startWith?: "home" | "lastMode";
  mouseRepeatStopEdge: "press" | "release";
  scanPreferences?: import("./preferences").ScanPreferences;
  wordPrediction: boolean;
  keyboardLayout: KeyboardLayout;
  keyboardWaitAfterTyping: boolean;
  panelAvoidsPointer: boolean;
  scannerColor: ScannerColor;
  mode: "line" | "grid";
  automatic: boolean;
  speed: number;
  /** The radar's speed on the line speed scale. Null, or missing in older saved state, follows `speed`. */
  radarSpeed?: number | null;
  gridSize: number;
  blockIntervalMs: number;
  autoSelectEnabled: boolean;
  autoSelectDelayMs: number;
  selectKey: string;
  nextKey: string;
  backKey: string;
  pauseKey: string;
};
/** Select opens Home unless it continues after an action or the user chose the last mode. */
export function selectOpensHome(config: PointScanConfig) {
  return config.startWith !== "lastMode";
}
export function lastModeName(config: PointScanConfig) {
  return config.controlMode === "mouse" ? "Mouse scanning" : "Point scanning";
}
export function mouseRepeatStopInstruction(config: PointScanConfig) {
  return config.mouseRepeatStopEdge === "press" ? "Press a switch to stop" : "Press and release a switch to stop";
}
type OutsideField = {
  get: (config: PointScanConfig) => unknown;
  set: (config: PointScanConfig, value: unknown) => PointScanConfig;
};
const topLevel = (key: "autoSelectEnabled" | "speed" | "mode" | "blockIntervalMs"): OutsideField => ({
  get: (config) => config[key],
  set: (config, value) => ({ ...config, [key]: value }),
});
/**
 * Values that change outside the settings form: Auto select from a switch,
 * and the scan rate, line speed and Line/Grid from the Scanning menu. An
 * unsaved form takes a change to one of these, so its next save does not
 * undo it.
 */
const outsideFields: OutsideField[] = [
  topLevel("autoSelectEnabled"),
  topLevel("speed"),
  topLevel("mode"),
  topLevel("blockIntervalMs"),
  {
    // The menu's own scan rate, when it has one.
    get: (config) => config.scanPreferences?.menu?.intervalMs,
    set: (config, value) => {
      if (typeof value !== "number") return config;
      const preferences = config.scanPreferences ?? defaultScanPreferences;
      return { ...config, scanPreferences: { ...preferences, menu: { ...preferences.menu, intervalMs: value } } };
    },
  },
];
/** The form with every outside change in `next` that is not an echo of the save in flight. */
function withOutsideChanges(
  form: PointScanConfig,
  next: PointScanConfig,
  previous: PointScanConfig,
  inFlight: PointScanConfig | null,
) {
  return outsideFields.reduce((merged, field) => {
    const value = field.get(next);
    const changed = value !== field.get(previous) && (!inFlight || value !== field.get(inFlight));
    return changed ? field.set(merged, value) : merged;
  }, form);
}
export type PointScanState = {
  remote?: boolean;
  config: PointScanConfig;
  enabled: boolean;
  phase: "autoSelecting" | "idle" | "row" | "rowEscape" | "cell" | "x" | "y" | "menu" | "menuSuspended" | "dragDestination" | "dragConfirmation" | "executing" | "keyboard" | "keyboardSuspended" | "keyboardOpening" | "keyboardRepeating" | "mouse" | "mouseSuspended" | "mouseMoving" | "mouseScrolling" | "mouseRadar";
  paused: boolean;
  message: string;
  supported: boolean;
};
export const defaultPointScanConfig: PointScanConfig = {
  controlMode: "point",
  startWith: "home",
  mouseRepeatStopEdge: "release",
  wordPrediction: true,
  keyboardLayout: "qwerty",
  keyboardWaitAfterTyping: false,
  panelAvoidsPointer: false,
  scannerColor: "blue",
  mode: "line",
  automatic: true,
  speed: 2,
  radarSpeed: null,
  gridSize: 4,
  blockIntervalMs: 1000,
  autoSelectEnabled: false,
  autoSelectDelayMs: 1000,
  selectKey: "Space",
  nextKey: "Enter",
  backKey: "Backspace",
  pauseKey: "F8",
};

export function validSwitches(config: PointScanConfig) {
  return (
    new Set([
      config.selectKey,
      config.nextKey,
      config.backKey,
      config.pauseKey,
      "Escape",
    ]).size === 5
  );
}
// App owns this hook so changing tabs or views never drops edits or stops a scan.
// Scanning has no toggle: the backend arms it whenever the saved switches allow
// and reports why not through the view's message.
export function useScanning() {
  const [state, setState] = useState<PointScanState | null>(null);
  const [config, setConfig] = useState(defaultPointScanConfig);
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const model = useRef({
    config: defaultPointScanConfig,
    revision: 0,
    saved: 0,
    pending: 0,
    supported: false,
    latestRuntime: null as PointScanState | null,
    /** The config of the save in flight, so its echo is not taken for an outside change. */
    inFlight: null as PointScanConfig | null,
  });
  const queue = useRef(Promise.resolve());
  const runtimeRevision = useRef(0);
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    const receive = (next: PointScanState) => {
      if (!alive) return;
      const previous = model.current.latestRuntime;
      model.current.supported = next.supported;
      model.current.latestRuntime = next;
      setState(next);
      if (
        model.current.pending === 0 &&
        model.current.revision === model.current.saved
      ) {
        model.current.config = next.config;
        setConfig(next.config);
      } else if (previous) {
        const merged = withOutsideChanges(model.current.config, next.config, previous.config, model.current.inFlight);
        if (merged !== model.current.config) {
          model.current.config = merged;
          setConfig(merged);
        }
      }
    };
    if (!("__TAURI_INTERNALS__" in window)) {
      receive({
        config: defaultPointScanConfig,
        enabled: false,
        phase: "idle",
        paused: false,
        supported: false,
        message: "Open the desktop app to use point scan.",
      });
      return;
    }
    void listen<PointScanState>("point-scan-changed", (event) => {
      runtimeRevision.current++;
      receive(event.payload);
    })
      .then((unlisten) => {
        if (alive) stop = unlisten;
        else unlisten();
        const revision = runtimeRevision.current;
        return invoke<PointScanState>("get_point_scan").then((next) => {
          if (revision === runtimeRevision.current) receive(next);
        });
      })
      .catch((reason) => {
        if (alive) setError(String(reason));
      });
    return () => {
      alive = false;
      stop?.();
    };
  }, []);
  const enqueue = (work: () => Promise<void>) => {
    model.current.pending++;
    setPending(model.current.pending);
    queue.current = queue.current
      .then(work)
      .catch((reason) => setError(String(reason)))
      .finally(() => {
        model.current.pending--;
        setPending(model.current.pending);
        const latest = model.current.latestRuntime;
        if (model.current.pending === 0 && model.current.revision === model.current.saved && latest) {
          model.current.config = latest.config;
          setConfig(latest.config);
        }
      });
  };
  const save = (revision: number) =>
    enqueue(async () => {
      if (revision !== model.current.revision) return;
      setError(null);
      const runtime = runtimeRevision.current;
      // The form as it is now: the edit plus any outside change made while
      // this save waited in the queue.
      const next = model.current.config;
      model.current.inFlight = next;
      let result: PointScanState;
      try {
        result = await invoke<PointScanState>("configure_point_scan", {
          config: next,
        });
      } finally {
        model.current.inFlight = null;
      }
      model.current.saved = revision;
      if (runtime === runtimeRevision.current) {
        setState(result);
        model.current.latestRuntime = result;
      }
      if (revision === model.current.revision) {
        model.current.config = result.config;
        setConfig(result.config);
      }
    });
  const update = <K extends keyof PointScanConfig>(
    key: K,
    value: PointScanConfig[K],
  ) => {
    const m = model.current;
    if (!m.supported) return;
    const next = { ...m.config, [key]: value };
    m.config = next;
    m.revision++;
    setConfig(next);
    setError(null);
    if (validSwitches(next)) save(m.revision);
  };
  const retry = () => {
    if (validSwitches(model.current.config))
      save(model.current.revision);
  };
  const chooseKeyboardLayout = (layout: KeyboardLayout) => {
    enqueue(async () => {
      setError(null);
      const result = await invoke<PointScanState>("set_keyboard_layout", { layout });
      model.current.latestRuntime = result;
      const next = model.current.revision === model.current.saved
        ? result.config
        : { ...model.current.config, keyboardLayout: result.config.keyboardLayout };
      model.current.config = next;
      setConfig(next);
      setState(result);
    });
  };
  return {
    state,
    config,
    pending,
    error,
    update,
    chooseKeyboardLayout,
    retry,
    unsaved: model.current.revision !== model.current.saved,
  };
}
export type ScanningController = ReturnType<typeof useScanning>;
