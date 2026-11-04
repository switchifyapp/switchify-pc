import { useEffect, useState } from "react";
import modelUrl from "./model/find-model.bin?url";
import { prepareMeaning, type SemanticIndex } from "./index";
import { unpackModel } from "./semantic";

let pending: Promise<SemanticIndex | undefined> | undefined;
function load(): Promise<SemanticIndex | undefined> {
  // Cache both success and failure. A broken asset must not cause a request per keystroke.
  return pending ??= (async () => {
    try {
      const response = await fetch(modelUrl);
      if (!response.ok) return undefined;
      return prepareMeaning(unpackModel(await response.arrayBuffer()));
    } catch { return undefined; }
  })();
}

export function useSemantic(query: string): SemanticIndex | undefined {
  const [semantic, setSemantic] = useState<SemanticIndex>();
  const active = !!query.trim();
  useEffect(() => {
    if (!active) return;
    let mounted = true;
    void load().then((model) => { if (mounted && model) setSemantic(model); });
    return () => { mounted = false; };
  }, [active]);
  return semantic;
}
