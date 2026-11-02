import type { CustomPanel, PanelAction, PanelCell, PictureCell } from "../types";
import type { PanelTheme } from "../scanning/preferences";
import { lightPanels } from "../scanning/preferences";

/** The native panels' colours, so a picture looks like the panel will. */
const palettes = {
  dark: { panel: "#141820", band: "#1e2430", utility: "#222936", toolbar: "#191e28", border: "#3e4859", bandBorder: "#4a566c", text: "#ffffff" },
  light: { panel: "#f0f2f6", band: "#e2e6ed", utility: "#eceff4", toolbar: "#f4f6f9", border: "#c4cbd6", bandBorder: "#b0b9c8", text: "#141820" },
} as const;

/** Back and Close tiles are drawn like the panels' toolbar keys. */
export function toolbarAction(action: PanelAction) {
  return action.type === "back" || action.type === "close";
}

export function picturePalette(theme: PanelTheme | undefined) {
  return lightPanels(theme) ? palettes.light : palettes.dark;
}

/** A panel drawn as the scan will show it: a title band, then the rows of
 *  tiles sharing each row's width by span. A native panel's own keys
 *  (`fixed`) are drawn faintly, with the user's rows above the last of
 *  them, where the panel scans them.
 *  Purely visual; nothing scans. */
export function PanelPicture({ panel, fixed, columns = [], theme, title = "Switchify PC", className = "" }: { panel: CustomPanel; fixed?: PictureCell[][]; columns?: PanelCell[][]; theme?: PanelTheme; title?: string; className?: string }) {
  const colours = picturePalette(theme);
  // The name says what is on the panel, row by row, since the tiles are decoration here.
  const label = (cell: PanelCell) => cell.label.trim() || "unlabelled";
  const rows = panel.rows.map((row) => row.map(label).join(", ")).join("; ");
  const columnsText = columns.length > 0 ? `; columns: ${columns.map((column) => column.map(label).join(", ")).join("; ")}` : "";
  const description = (fixed ? (rows ? `its keys, then ${rows}` : "its keys only") : rows) + columnsText;
  // The picture's lines, top to bottom: the keys, the rows below them, the
  // last row of keys, then any lines a long column adds. Each line keeps
  // the keys' width clear and ends with a slot per column.
  const keysWeight = Math.max(1, ...(fixed ?? []).map((row) => row.reduce((sum, key) => sum + key.weight, 0)));
  const columnWidth = (column: PanelCell[]) => Math.max(1, ...column.map((cell) => cell.span));
  type Line = { key: string; keys: React.ReactNode[]; blank: boolean };
  const lines: Line[] = [
    ...(fixed ?? []).slice(0, -1).map((row, index): Line => ({ key: `fixed-${index}`, blank: false, keys: row.map((key, column) => <FixedTile key={column} cell={key} colours={colours} />) })),
    ...panel.rows.map((row, index): Line => ({ key: `row-${index}`, blank: false, keys: row.map((cell, column) => <PictureTile key={column} cell={cell} colours={colours} />) })),
    ...(fixed ?? []).slice(-1).map((row): Line => ({ key: "fixed-last", blank: false, keys: row.map((key, column) => <FixedTile key={column} cell={key} colours={colours} />) })),
  ];
  const longest = Math.max(0, ...columns.map((column) => column.length));
  for (let index = lines.length; index < longest; index++) lines.push({ key: `below-${index}`, blank: true, keys: [] });
  return <div className={`panel-picture ${className}`.trim()} role="img" aria-label={`Picture of ${panel.name}: ${description}`} style={{ background: colours.panel, borderColor: colours.border, color: colours.text }}>
    <div className="panel-picture-band" style={{ background: colours.band, borderColor: colours.bandBorder }}><span>{title}</span></div>
    <div className="panel-picture-rows">
      {lines.map((line, index) => <div className="panel-picture-row" key={line.key}>
        {line.blank ? <span className="panel-picture-spacer" style={{ flexGrow: keysWeight, flexBasis: 0 }} /> : line.keys}
        {columns.map((column, c) => column[index]
          ? <PictureTile key={`column-${c}`} cell={column[index]} colours={colours} width={columnWidth(column)} data-column={c + 1} />
          : <span key={`column-${c}`} className="panel-picture-spacer" style={{ flexGrow: columnWidth(column), flexBasis: 0 }} />)}
      </div>)}
    </div>
  </div>;
}

function FixedTile({ cell, colours }: { cell: PictureCell; colours: (typeof palettes)[keyof typeof palettes] }) {
  return <span className="panel-picture-tile" data-fixed="true" style={{ flexGrow: cell.weight, flexBasis: 0, background: colours.utility, borderColor: colours.border }}>{cell.label}</span>;
}

function PictureTile({ cell, colours, width, "data-column": column }: { cell: PanelCell; colours: (typeof palettes)[keyof typeof palettes]; width?: number; "data-column"?: number }) {
  const toolbar = toolbarAction(cell.action);
  return <span className="panel-picture-tile" data-toolbar={toolbar || undefined} data-column={column} style={{ flexGrow: width ?? cell.span, flexBasis: 0, background: toolbar ? colours.toolbar : colours.utility, borderColor: colours.border }}>{cell.label}</span>;
}
