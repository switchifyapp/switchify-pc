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

// 1. Status-only entries never count as "changed"; 7. strip labels match the headings; 9. duplicate branch; 11. built-in section only when it renders.
edit("src/find/index.ts", [
  [`  /** Whether the setting exists on this computer. */
  when?: (ctx: FindContext) => boolean;
};`, `  /** Whether the setting exists on this computer. */
  when?: (ctx: FindContext) => boolean;
  /** Reports state rather than holding a choice, so a change is never the user's. */
  status?: true;
};`],
  [`    { id: "starting", label: "Select starts" }, { id: "areas", label: "Point and Mouse" }, { id: "movement", label: "Movement" }, { id: "after", label: "After a selection" },`, `    { id: "starting", label: "Starting" }, { id: "areas", label: "Point and Mouse" }, { id: "movement", label: "Movement" }, { id: "after", label: "After a selection" },`],
  [`  panels: [{ id: "yours", label: "Your panels" }, { id: "builtin", label: "Built-in menus" }],`, `  panels: [{ id: "yours", label: "Your panels" }, { id: "builtin", label: "Built-in menus", when: (c) => c.builtins.length > 0 }],`],
  [`  { id: "practice", label: "Test switches", words: ["practice", "try", "check my switch", "working", "safe"], page: "switches", section: "practice",
    value:`, `  { id: "practice", label: "Test switches", words: ["practice", "try", "check my switch", "working", "safe"], page: "switches", section: "practice", status: true,
    value:`],
  [`  { id: "builtin-menus", label: "Built-in menus and panels", words: ["home menu", "editing", "browser", "rearrange", "reset", "order of tiles", "menu tiles", "more actions", "keyboard rows"], page: "panels", section: "builtin",
    value:`, `  { id: "builtin-menus", label: "Built-in menus and panels", words: ["home menu", "editing", "browser", "rearrange", "reset", "order of tiles", "menu tiles", "more actions", "keyboard rows"], page: "panels", section: "builtin", when: (c) => c.builtins.length > 0,
    value:`],
  [`page: "mobile", section: "connection",
    value:`, `page: "mobile", section: "connection", status: true,
    value:`],
  [`page: "mobile", section: "pairing",
    value:`, `page: "mobile", section: "pairing", status: true,
    value:`],
  [`page: "mobile", section: "devices",
    value:`, `page: "mobile", section: "devices", status: true,
    value:`],
  [`page: "settings", section: "updates",
    value:`, `page: "settings", section: "updates", status: true,
    value:`],
  [`page: "settings", section: "licence",
    value:`, `page: "settings", section: "licence", status: true,
    value:`],
  [`page: "settings", section: "about",
    value:`, `page: "settings", section: "about", status: true,
    value:`],
  [`page: "support", section: "setup",
    value:`, `page: "support", section: "setup", status: true,
    value:`],
  [`page: "support", section: "fix",
    value: () => "Use a switch interface in keyboard mode"`, `page: "support", section: "fix", status: true,
    value: () => "Use a switch interface in keyboard mode"`],
  [`page: "support", section: "fix",
    value: (c) => c.state.diagnostics`, `page: "support", section: "fix", status: true,
    value: (c) => c.state.diagnostics`],
  [`      else if (e.words.includes(t)) total += 5;
      else if (e.words.includes(t)) total += 5;`, `      else if (e.words.includes(t)) total += 5;`],
]);

// 1. useRecent skips status entries; 5. live count; 6. the tile focuses its input.
edit("src/find/Find.tsx", [
  [`      const changed = index.filter((e) => last.current![e.id] != null && now[e.id] != null && now[e.id] !== last.current![e.id]).map((e) => e.id);`, `      const changed = index.filter((e) => !e.status && last.current![e.id] != null && now[e.id] != null && now[e.id] !== last.current![e.id]).map((e) => e.id);`],
  [`  return <div className={query ? "tile tile-find tile-chosen" : "tile tile-find"} role="search">`, `  // The whole tile is the target: a press anywhere on it focuses the box.
  return <div className={query ? "tile tile-find tile-chosen" : "tile tile-find"} role="search" onClick={() => inputRef.current?.focus()}>`],
  [`  const hits = search(query, ctx);
  return <div className="view">
    <header className="page-header"><div>
      <h1>{hits.length ? \`\${hits.length} \${hits.length === 1 ? "setting matches" : "settings match"} “\${query}”\` : \`Nothing matches “\${query}”\`}</h1>`, `  const hits = search(query, ctx);
  const title = hits.length ? \`\${hits.length} \${hits.length === 1 ? "setting matches" : "settings match"} “\${query}”\` : \`Nothing matches “\${query}”\`;
  return <div className="view">
    <p className="sr-only" role="status" aria-live="polite">{title}</p>
    <header className="page-header"><div>
      <h1>{title}</h1>`],
]);

// 2. no top-scroll while Updates is being focused; 3. the notice never speaks while Find covers the page; 4. setup copy; stale comment.
edit("src/App.tsx", [
  [`  useEffect(() => { if (!arrival) document.scrollingElement?.scrollTo?.({ top: 0 }); }, [view, query]);`, `  useEffect(() => { if (!arrival && !focusUpdates) document.scrollingElement?.scrollTo?.({ top: 0 }); }, [view, query]);`],
  [`    if (view !== "settings") return;`, `    // Find's results replace the page without changing the view, so they are
    // the one time the Updates section is off screen while on Settings.
    if (view !== "settings" || query) return;`],
  [`  }, [updateFailure, updatesShown, view]);`, `  }, [updateFailure, updatesShown, view, query]);`],
  [`stop on switch press or release under Mouse settings.`, `stop on switch press or release under Scanning, Mouse pointer.`],
]);

// 7. headings match the strip.
edit("src/settings/ScannerPreferences.tsx", [
  [`<section className="scanner-areas" aria-label="Scanning modes" data-section="areas">`, `<section className="scanner-areas" aria-label="Point and Mouse" data-section="areas">`],
  [`<h2 id="scanner-areas-title">Advanced panel settings</h2>`, `<h2 id="scanner-areas-title">Menus and keyboard</h2>`],
]);

// 6. 64px clear button; 12. forced colours for the active Find tile.
edit("src/ui/app.css", [
  [`.btn.find-clear { min-width: 56px; min-height: 56px; padding: 0; }`, `.btn.find-clear { min-width: 64px; min-height: 64px; padding: 0; }`],
  [`.tile-find:focus-within, [data-found] { outline: 3px solid Highlight; outline-offset: -3px; }`, `.tile-find:focus-within, .tile-find.tile-chosen, [data-found] { outline: 3px solid Highlight; outline-offset: -3px; }`],
]);

// 8. docs nits.
edit("site/src/content/docs/account-and-privacy.md", [["the **Privacy** tab", "the **Privacy** section of Settings"]]);
console.log("done");
