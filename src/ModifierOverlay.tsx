import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useLayoutEffect, useState } from "react";
import "./modifier-overlay.css";

export type ModifierOverlaySnapshot = {
  revision: number;
  labels: string[];
};

const emptySnapshot: ModifierOverlaySnapshot = { revision: 0, labels: [] };

export function isModifierOverlayRoute(search: string): boolean {
  return new URLSearchParams(search).get("view") === "modifier-overlay";
}

export function newestModifierSnapshot(
  current: ModifierOverlaySnapshot,
  incoming: ModifierOverlaySnapshot,
): ModifierOverlaySnapshot {
  return incoming.revision >= current.revision ? incoming : current;
}

export function ModifierOverlayView({ snapshot }: { snapshot: ModifierOverlaySnapshot }) {
  return (
    <div
      className="modifier-overlay-canvas"
      data-empty={snapshot.labels.length === 0}
      aria-hidden={snapshot.labels.length === 0}
    >
      <div className="modifier-overlay-panel" role="status" aria-label="Active modifiers">
        {snapshot.labels.map((label) => (
          <span className="modifier-overlay-chip" key={label}>{label}</span>
        ))}
      </div>
    </div>
  );
}

export function ModifierOverlay() {
  const [snapshot, setSnapshot] = useState<ModifierOverlaySnapshot>(emptySnapshot);

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    const apply = (incoming: ModifierOverlaySnapshot) => {
      if (!disposed) setSnapshot((current) => newestModifierSnapshot(current, incoming));
    };

    void listen<ModifierOverlaySnapshot>("modifier-overlay-changed", (event) => {
      apply(event.payload);
    }).then((removeListener) => {
      if (disposed) {
        removeListener();
        return;
      }
      unlisten = removeListener;
      void invoke<ModifierOverlaySnapshot>("modifier_overlay_ready").then(apply).catch(() => undefined);
    }).catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useLayoutEffect(() => {
    const window = getCurrentWindow();
    void (snapshot.labels.length > 0 ? window.show() : window.hide()).catch(() => undefined);
  }, [snapshot]);

  return <ModifierOverlayView snapshot={snapshot} />;
}
