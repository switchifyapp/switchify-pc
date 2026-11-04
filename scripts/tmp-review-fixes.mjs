import fs from "node:fs";
const edit = (p, pairs) => {
  let s = fs.readFileSync(p, "utf8");
  const nl = s.includes("\r\n") ? "\r\n" : "\n";
  for (let [a, b] of pairs) {
    a = a.split("\n").join(nl); b = b.split("\n").join(nl);
    if (!s.includes(a)) { console.error("MISSING in " + p + ": " + a.slice(0, 80)); process.exitCode = 1; continue; }
    s = s.split(a).join(b);
  }
  fs.writeFileSync(p, s);
};

// CI fetches the model before validating the frontend.
edit(".github/workflows/ci.yml", [[`      - run: npm run lint
      - run: npm test
      - run: npm run build

  changes:`, `      - name: Fetch find model
        run: node scripts/fetch-find-model.mjs
      - run: npm run lint
      - run: npm test
      - run: npm run build

  changes:`]]);
edit(".github/workflows/release-tauri.yml", [[`      - name: Validate frontend at release tag
        run: |
          npm run lint`, `      - name: Fetch find model
        run: node scripts/fetch-find-model.mjs

      - name: Validate frontend at release tag
        run: |
          npm run lint`]]);

// A missing asset degrades to keyword-only instead of breaking the build.
edit("src/find/useSemantic.ts", [[`import { useEffect, useState } from "react";
import modelUrl from "./model/find-model.bin?url";
import { prepareMeaning, type SemanticIndex } from "./index";
import { unpackModel } from "./semantic";

let pending: Promise<SemanticIndex | undefined> | undefined;
function load(): Promise<SemanticIndex | undefined> {
  // Cache both success and failure. A broken asset must not cause a request per keystroke.
  return pending ??= (async () => {
    try {
      const response = await fetch(modelUrl);`, `// The model file is fetched at build time (scripts/fetch-find-model.mjs). A
// checkout without it still builds and tests: Find then matches by keyword only.
import { useEffect, useState } from "react";
import { prepareMeaning, type SemanticIndex } from "./index";
import { unpackModel } from "./semantic";

const assets = import.meta.glob<string>("./model/find-model.bin", { query: "?url", import: "default", eager: true });
const modelUrl: string | undefined = Object.values(assets)[0];

let pending: Promise<SemanticIndex | undefined> | undefined;
function load(): Promise<SemanticIndex | undefined> {
  // Cache both success and failure. A broken asset must not cause a request per keystroke.
  return pending ??= (async () => {
    try {
      if (!modelUrl) return undefined;
      const response = await fetch(modelUrl);`]]);
edit("vite.config.ts", [[`import { readFileSync } from "node:fs";`, `import { existsSync, readFileSync } from "node:fs";`],
  [`      for (const name of ["LICENSE", "NOTICE"]) this.emitFile({ type: "asset", fileName: "assets/find-model." + name, source: readFileSync(new URL("./src/find/model/" + name, import.meta.url)) });`,
   `      // Shipped beside the model; absent (with the model) on a checkout that has not fetched it.
      for (const name of ["LICENSE", "NOTICE"]) {
        const file = new URL("./src/find/model/" + name, import.meta.url);
        if (existsSync(file)) this.emitFile({ type: "asset", fileName: "assets/find-model." + name, source: readFileSync(file) });
      }`]]);

// Accents: every combining mark, written as a Unicode property rather than invisible characters.
edit("src/find/wordpiece.ts", [[`  const clean = text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();`, `  const clean = text.normalize("NFD").replace(/\\p{Mn}/gu, "").toLowerCase();`]]);

// Rows must line up with ids.
edit("scripts/fetch-find-model.mjs", [[`  const words = [...vocab.entries()].sort((a, b) => a[1] - b[1]).map(([word]) => word);`, `  const words = [...vocab.entries()].sort((a, b) => a[1] - b[1]).map(([word]) => word);
  // Row i of the matrix is token i, so the ids must be exactly 0..n-1.
  words.forEach((word, i) => { if (vocab.get(word) !== i) throw new Error('Find model vocabulary ids are not contiguous'); });`]]);

// Meaning adds a short list, not a long tail.
edit("src/find/index.ts", [[`export const meaningFloor = 0.35;`, `export const meaningFloor = 0.35;
/** How many meaning-only matches follow the keyword hits: enough to catch the setting, few enough to scan. */
export const meaningLimit = 8;`],
  [`  }).filter(({ score }) => score >= meaningFloor).sort((a, b) => b.score - a.score);
  return [...keyword, ...rest.map(({ e }) => e)];`, `  }).filter(({ score }) => score >= meaningFloor).sort((a, b) => b.score - a.score).slice(0, meaningLimit);
  return [...keyword, ...rest.map(({ e }) => e)];`]]);

// Header comments first.
edit("src/find/Find.tsx", [[`import { useSemantic } from "./useSemantic";
// Finding things: the Find tile and its results, the section strip at the
// top of each page, the Also see links, Home's common tasks and recently
// changed, and the jump that takes a person to one setting and marks it.
import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";`, `// Finding things: the Find tile and its results, the section strip at the
// top of each page, the Also see links, Home's common tasks and recently
// changed, and the jump that takes a person to one setting and marks it.
import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { useSemantic } from "./useSemantic";`]]);
edit("src/find/index.ts", [[`import { embed, similarity, type StaticModel } from "./semantic";
// The settings index: one entry per thing a person might look for, with the
// words they might use for it, where it lives, and what it is set to now.
// Find, Home's common tasks and recently changed, each page's section strip
// and the Also see links all read from this, so there is one map of the app.
import type { AccountView, AppSettings, AppState, BuiltinPanelView, CustomPanel, SwitchProfile } from "../types";`, `// The settings index: one entry per thing a person might look for, with the
// words they might use for it, where it lives, and what it is set to now.
// Find, Home's common tasks and recently changed, each page's section strip
// and the Also see links all read from this, so there is one map of the app.
import { embed, similarity, type StaticModel } from "./semantic";
import type { AccountView, AppSettings, AppState, BuiltinPanelView, CustomPanel, SwitchProfile } from "../types";`]]);
console.log("done");
