// The model file is fetched at build time (scripts/fetch-find-model.mjs). A
// checkout without it still builds and tests: Find then matches by keyword only.
import { useEffect, useState } from "react";
import { prepareMeaning, type SemanticIndex } from "./index";
import { unpackModel } from "./semantic";

// No type argument here: Vite only rewrites the plain call form.
const assets = import.meta.glob("./model/find-model.bin", { query: "?url", import: "default", eager: true }) as Record<string, string>;
const modelUrl: string | undefined = Object.values(assets)[0];
/** Whether this build carries the model at all; tests that need a fetch skip without it. */
export const hasModelAsset = !!modelUrl;

let pending: Promise<SemanticIndex | undefined> | undefined;
function load(): Promise<SemanticIndex | undefined> {
  // Cache both success and failure. A broken asset must not cause a request per keystroke.
  return pending ??= (async () => {
    try {
      if (!modelUrl) return undefined;
      const response = await fetch(modelUrl);
      if (!response.ok) return undefined;
      return prepareMeaning(unpackModel(await response.arrayBuffer()));
    } catch { return undefined; }
  })();
}

export function useSemantic(query: string): { semantic: SemanticIndex | undefined; loading: boolean } {
  const [semantic, setSemantic] = useState<SemanticIndex>();
  const [settled, setSettled] = useState(false);
  const active = !!query.trim();
  useEffect(() => {
    if (!active) return;
    let mounted = true;
    void load().then((model) => {
      if (mounted) { setSemantic(model); setSettled(true); }
    });
    return () => { mounted = false; };
  }, [active]);
  return { semantic, loading: active && hasModelAsset && !settled };
}
