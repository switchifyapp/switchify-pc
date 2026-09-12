import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export type PointScanConfig = {
  mode: "line" | "grid";
  automatic: boolean;
  speed: number;
  gridSize: number;
  blockIntervalMs: number;
  selectKey: string;
  nextKey: string;
  backKey: string;
  pauseKey: string;
};
export type PointScanState = {
  config: PointScanConfig;
  enabled: boolean;
  phase: "idle" | "row" | "cell" | "x" | "y";
  paused: boolean;
  message: string;
  supported: boolean;
};
export const defaultPointScanConfig: PointScanConfig = {
  mode: "line",
  automatic: true,
  speed: 2,
  gridSize: 4,
  blockIntervalMs: 1000,
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
export function useScanning() {
  const [state, setState] = useState<PointScanState | null>(null);
  const [config, setConfig] = useState(defaultPointScanConfig);
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [toggling, setToggling] = useState(false);
  const model = useRef({
    config: defaultPointScanConfig,
    revision: 0,
    saved: 0,
    pending: 0,
    enabled: false,
    toggling: false,
    supported: false,
  });
  const queue = useRef(Promise.resolve());
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    const receive = (next: PointScanState) => {
      if (!alive) return;
      model.current.enabled = next.enabled;
      model.current.supported = next.supported;
      setState(next);
      if (
        model.current.pending === 0 &&
        model.current.revision === model.current.saved
      ) {
        model.current.config = next.config;
        setConfig(next.config);
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
    void listen<PointScanState>("point-scan-changed", (event) =>
      receive(event.payload),
    )
      .then((unlisten) => {
        if (alive) stop = unlisten;
        else unlisten();
        return invoke<PointScanState>("get_point_scan");
      })
      .then(receive)
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
      });
  };
  const save = (revision: number, next: PointScanConfig) =>
    enqueue(async () => {
      if (revision !== model.current.revision) return;
      setError(null);
      const result = await invoke<PointScanState>("configure_point_scan", {
        config: next,
        enabled: false,
      });
      model.current.saved = revision;
      model.current.enabled = result.enabled;
      setState(result);
    });
  const update = <K extends keyof PointScanConfig>(
    key: K,
    value: PointScanConfig[K],
  ) => {
    const m = model.current;
    if (m.enabled || m.toggling || !m.supported) return;
    const next = { ...m.config, [key]: value };
    m.config = next;
    m.revision++;
    setConfig(next);
    setError(null);
    if (validSwitches(next)) save(m.revision, next);
  };
  const retry = () => {
    if (validSwitches(model.current.config))
      save(model.current.revision, model.current.config);
  };
  const toggle = () => {
    const m = model.current;
    if (m.toggling || !m.supported || (!m.enabled && !validSwitches(m.config)))
      return;
    const enabled = !m.enabled;
    m.toggling = true;
    setToggling(true);
    enqueue(async () => {
      try {
        if (enabled && m.saved !== m.revision)
          throw new Error(
            "Save the scanning settings before enabling point scan. Use Retry save.",
          );
        setError(null);
        const result = await invoke<PointScanState>("configure_point_scan", {
          config: m.config,
          enabled,
        });
        m.enabled = result.enabled;
        setState(result);
      } finally {
        m.toggling = false;
        setToggling(false);
      }
    });
  };
  return {
    state,
    config,
    pending,
    error,
    toggling,
    update,
    retry,
    toggle,
    unsaved: model.current.revision !== model.current.saved,
  };
}
export type ScanningController = ReturnType<typeof useScanning>;
