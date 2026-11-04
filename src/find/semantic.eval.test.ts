// Measures what meaning-based matching adds, on real-sounding queries, so the
// model size is a decision rather than a guess. Runs only when FIND_EVAL
// names a directory holding potion-base-* folders (tokenizer.json and
// model.safetensors each); the normal test run skips it.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { browserState } from "../api";
import { defaultPointScanConfig } from "../scanning/useScanning";
import { embed, similarity, type StaticModel } from "./semantic";
import { vocabularyFromTokenizerJson } from "./wordpiece";
import { entryText, index, search, type FindContext } from "./index";

const root = process.env.FIND_EVAL;

/** Queries as people write them, with the entry each should reach. */
export const evalQueries: [string, string][] = [
  ["my scan is too quick", "scan-rate"], ["scanning goes too fast for me", "scan-rate"], ["slow the highlight down", "scan-rate"], ["how long each item waits", "scan-rate"],
  ["the pointer moves too far each step", "pointer-speed"], ["mouse goes too fast", "pointer-speed"], ["cursor jumps a lot", "pointer-speed"],
  ["make the box red", "colour"], ["the highlight is hard to see", "thickness"], ["thicker border", "thickness"], ["change the colour of the scanning box", "colour"],
  ["letters in a different order", "keyboard-layout"], ["qwerty keyboard", "keyboard-layout"], ["word suggestions while typing", "word-prediction"], ["stop suggesting words", "word-prediction"],
  ["connect my phone", "connection"], ["pair a tablet", "pairing"], ["how do I link my android", "pairing"], ["remove an old phone", "devices"], ["forget a device", "devices"],
  ["the ring that shows where the mouse is", "overlay"], ["crosshairs", "overlay"], ["what does my switch do", "switch-list"], ["add a new button", "switch-list"], ["press and hold actions", "switch-list"],
  ["how long to hold the switch", "hold-interval"], ["check my switch works", "practice"], ["my switch does nothing", "not-recognised"], ["switch is not detected", "not-recognised"],
  ["open when windows starts", "startup"], ["run at login", "startup"], ["click automatically when I stop moving", "dwell"], ["dwell clicking", "dwell"],
  ["is there a new version", "updates"], ["how much does it cost", "licence"], ["enter my key", "licence"], ["pay for the app", "licence"],
  ["log in to my account", "account"], ["sync settings to my other computer", "account"], ["send anonymous usage data", "privacy"], ["privacy", "privacy"],
  ["bigger buttons on the panels", "panel-size"], ["dark theme", "panel-theme"], ["light mode for the keyboard", "panel-theme"],
  ["make my own page of buttons", "your-panels"], ["change the home menu order", "builtin-menus"], ["rearrange the editing menu", "builtin-menus"],
  ["what happens after I click", "after-selection"], ["scan backwards", "direction"], ["stop after a few loops", "pass-limit"], ["rows then items", "item-scanning"],
  ["grid or line", "point-method"], ["click on its own after a delay", "auto-select"], ["radar", "radar"], ["panel gets in the way of the mouse", "panel-position"],
  ["stop the mouse repeating", "repeat"], ["keep scrolling while I hold", "repeat"], ["keys repeat on the keyboard", "key-repeat"],
  ["mac permission for control", "input-access"], ["accessibility permission", "input-access"], ["send a bug report", "diagnostics"], ["bluetooth is off", "bluetooth"], ["start the setup again", "setup-guide"],
  ["forward switches from my phone", "profiles"], ["profile for grid 3", "profiles"], ["approve my phone without the code", "auto-approve"],
];

function loadModel(dir: string): StaticModel {
  const vocab = vocabularyFromTokenizerJson(JSON.parse(readFileSync(join(dir, "tokenizer.json"), "utf8")));
  const bytes = readFileSync(join(dir, "model.safetensors"));
  const headerLength = Number(bytes.readBigUInt64LE(0));
  const header = JSON.parse(bytes.subarray(8, 8 + headerLength).toString()) as { embeddings: { shape: [number, number]; data_offsets: [number, number] } };
  const [size, dims] = header.embeddings.shape;
  const start = 8 + headerLength + header.embeddings.data_offsets[0];
  const vectors = new Float32Array(bytes.buffer.slice(bytes.byteOffset + start, bytes.byteOffset + start + size * dims * 4));
  return { vocab, dims, vectors, unkId: vocab.get("[UNK]") ?? -1 };
}

function context(): FindContext {
  const state = structuredClone(browserState);
  return { state, settings: state.settings, config: structuredClone(defaultPointScanConfig), scan: null, bindings: [{ id: "space", name: "Space switch", key: "Space", pressAction: "select", holdActions: [] }], holdIntervalMs: 1000, panels: [], builtins: [], profiles: [], account: { available: true, signedIn: true, email: "me@example.com", pendingEmail: null, keychainUnavailable: false } };
}

describe.skipIf(!root)("semantic matching", () => {
  for (const name of ["potion-base-2M", "potion-base-4M", "potion-base-8M"]) {
    it(`measures ${name}`, () => {
      const model = loadModel(join(root!, name));
      const ctx = context();
      const vectors = new Map(index.map((e) => [e.id, embed(model, entryText(e))]));
      const rank = (query: string, mode: "keyword" | "semantic" | "both") => {
        const keyword = search(query, ctx).map((e) => e.id);
        if (mode === "keyword") return keyword;
        const q = embed(model, query);
        const scored = index.map((e) => ({ id: e.id, s: q && vectors.get(e.id) ? similarity(q, vectors.get(e.id)!) : -1 })).sort((a, b) => b.s - a.s);
        if (mode === "semantic") return scored.map((x) => x.id);
        // Keyword hits first, in keyword order; then anything the meaning adds above a floor.
        const rest = scored.filter((x) => !keyword.includes(x.id) && x.s >= 0.35).map((x) => x.id);
        return [...keyword, ...rest];
      };
      const report: Record<string, { at1: number; at3: number; misses: string[] }> = {};
      for (const mode of ["keyword", "semantic", "both"] as const) {
        let at1 = 0, at3 = 0; const misses: string[] = [];
        for (const [query, expected] of evalQueries) {
          const ids = rank(query, mode);
          if (ids[0] === expected) at1++;
          if (ids.slice(0, 3).includes(expected)) at3++; else misses.push(`${query} -> ${ids.slice(0, 3).join(",") || "nothing"}`);
        }
        report[mode] = { at1, at3, misses };
      }
      const n = evalQueries.length;
      console.log(`\n${name} (${model.dims} dims), ${n} queries`);
      for (const [mode, r] of Object.entries(report)) console.log(`  ${mode.padEnd(8)} top1 ${r.at1}/${n}  top3 ${r.at3}/${n}`);
      console.log("  both misses:\n    " + report.both.misses.join("\n    "));
      expect(report.both.at3).toBeGreaterThanOrEqual(report.keyword.at3);
    });
  }
});
