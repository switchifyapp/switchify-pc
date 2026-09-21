import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SwitchAction } from "./useSwitches";

// Switches forwarded from Switchify Remote. Slots are positional: slot N is
// the Nth switch on the phone's Forwarding screen. The backend applies saved
// changes to a live remote session, so edits save as they happen, like keys.
export type RemoteSlot = {
  pressAction: SwitchAction | null;
  holdActions: SwitchAction[];
  name?: string;
};
export type RemoteConfig = {
  schemaVersion: number;
  revision: number;
  slots: RemoteSlot[];
};
export const remoteSlotCount = 8;

export function useRemoteSwitches() {
  const [config, setConfig] = useState<RemoteConfig | null>(null);
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const model = useRef({
    config: null as RemoteConfig | null,
    revision: 0,
    saved: 0,
    pending: 0,
  });
  const queue = useRef(Promise.resolve());
  useEffect(() => {
    let alive = true;
    if (!("__TAURI_INTERNALS__" in window)) return;
    void invoke<RemoteConfig>("get_remote_switches")
      .then((value) => {
        if (!alive) return;
        model.current.config = value;
        setConfig(value);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, []);
  const save = (revision: number, next: RemoteConfig) => {
    const m = model.current;
    m.pending++;
    setPending(m.pending);
    queue.current = queue.current
      .then(async () => {
        if (revision !== m.revision) return;
        setError(null);
        const result = await invoke<RemoteConfig>("save_remote_switches", {
          config: next,
        });
        m.saved = revision;
        if (m.config) {
          m.config = { ...m.config, revision: result.revision };
          setConfig(m.config);
        }
      })
      .catch((e) => setError(String(e)))
      .finally(() => {
        m.pending--;
        setPending(m.pending);
      });
  };
  const update = (slots: RemoteSlot[]) => {
    const m = model.current;
    if (!m.config) return;
    const next = { ...m.config, slots };
    m.config = next;
    m.revision++;
    setConfig(next);
    save(m.revision, next);
  };
  const remove = async (index: number) => {
    const m = model.current;
    if (!m.config || m.pending || m.revision !== m.saved) return false;
    const next = { ...m.config, slots: m.config.slots.map((slot, i) => i === index ? { pressAction: null, holdActions: [] } : slot) };
    m.pending++;
    setPending(m.pending);
    setError(null);
    try {
      const result = await invoke<RemoteConfig>("save_remote_switches", { config: next });
      m.config = result;
      setConfig(result);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      m.pending--;
      setPending(m.pending);
    }
  };
  return {
    config,
    pending,
    error,
    update,
    remove,
    retry: () => {
      if (model.current.config)
        save(model.current.revision, model.current.config);
    },
    unsaved: model.current.revision !== model.current.saved,
  };
}
export type RemoteSwitchController = ReturnType<typeof useRemoteSwitches>;
