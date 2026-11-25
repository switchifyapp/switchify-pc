import type { CustomPanel, PanelAction, PanelCell, PictureCell, TileColor } from "../types";
import type { PanelTheme } from "../scanning/preferences";
import { lightPanels } from "../scanning/preferences";

/** The colours a tile can be given, in the order they are offered. */
export const tileColors: { value: TileColor; label: string }[] = [
  { value: "red", label: "Red" }, { value: "orange", label: "Orange" }, { value: "yellow", label: "Yellow" }, { value: "green", label: "Green" },
  { value: "blue", label: "Blue" }, { value: "purple", label: "Purple" }, { value: "pink", label: "Pink" }, { value: "grey", label: "Grey" },
];

/** The native panels' colours, so a picture looks like the panel will.
 *  `tiles` are the faces of coloured tiles, as `tile_fill` draws them. */
const palettes: Record<"dark" | "light", Palette> = {
  dark: { panel: "#141820", band: "#1e2430", utility: "#222936", toolbar: "#191e28", border: "#3e4859", bandBorder: "#4a566c", text: "#ffffff",
    tiles: { red: "#962828", orange: "#9c5414", yellow: "#826a0a", green: "#1e703c", blue: "#1c5aa0", purple: "#683ea0", pink: "#9c306e", grey: "#68707e" } },
  light: { panel: "#f0f2f6", band: "#e2e6ed", utility: "#eceff4", toolbar: "#f4f6f9", border: "#c4cbd6", bandBorder: "#b0b9c8", text: "#141820",
    tiles: { red: "#ffaaa5", orange: "#ffc482", yellow: "#fae478", green: "#a0e1aa", blue: "#a0c8ff", purple: "#d0b8ff", pink: "#ffb4dc", grey: "#b0b7c2" } },
};
export type Palette = { panel: string; band: string; utility: string; toolbar: string; border: string; bandBorder: string; text: string; tiles: Record<TileColor, string> };

/** The face of a tile in a picture: its colour if it has one, otherwise the face its action gives. */
export function tileFace(cell: PanelCell, colours: Palette) {
  return cell.color ? colours.tiles[cell.color] : toolbarAction(cell.action) ? colours.toolbar : colours.utility;
}

/** Back and Close tiles are drawn like the panels' toolbar keys. */
export function toolbarAction(action: PanelAction) {
  return action.type === "back" || action.type === "close";
}

export function picturePalette(theme: PanelTheme | undefined) {
  return lightPanels(theme) ? palettes.light : palettes.dark;
}

/** Where a tile sits: in a row, or (`column` set) in a column, `row` being the column's index. */
export type TilePlace = { row: number; cell: number; column?: true };

/** Makes a picture editable: the user's tiles are drawn by `tile`, every
 *  row gets tools beside it and columns get labelled tools below. Wherever a
 *  tile can be created there is an add button: at the end of a row or column
 *  with room (`rowEnd`, `columnEnd`), a new row where it will land (`newRow`)
 *  and a new column to the right (`newColumn`). Each is null where there is
 *  no room. The whole thing is a group rather than an image. */
export type PictureEditing = {
  tile: (cell: PanelCell, place: TilePlace, colours: Palette) => React.ReactNode;
  rowTools: (row: number) => React.ReactNode;
  columnTools: (column: number) => React.ReactNode;
  rowEnd: (row: number) => React.ReactNode;
  columnEnd: (column: number) => React.ReactNode;
  newRow: React.ReactNode;
  newColumn: React.ReactNode;
};

/** A panel drawn as the scan will show it: a band with the panel's name, in
 *  the panel's colour (`accent`) if it has one, then the rows of
 *  tiles sharing each row's width by span. A native panel's own keys
 *  (`fixed`) are drawn faintly, with the user's rows above the last of
 *  them, where the panel scans them, and the user's columns down the right.
 *  Purely visual unless `editing` is given. */
export function PanelPicture({ panel, fixed, columns = [], editing, theme, accent, title = panel.name, className = "" }: { panel: CustomPanel; fixed?: PictureCell[][]; columns?: PanelCell[][]; editing?: PictureEditing; theme?: PanelTheme; accent?: TileColor | null; title?: string; className?: string }) {
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
  const userTile = (cell: PanelCell, place: TilePlace, width?: number) => editing
    ? editing.tile(cell, place, colours)
    : <PictureTile cell={cell} colours={colours} width={width} data-column={place.column ? place.row + 1 : undefined} />;
  type Line = { key: string; keys: React.ReactNode[]; blank: boolean; row?: number };
  // A row's add button sits after its tiles at a fixed width, so the tiles keep their shares of the row.
  const rowEnd = (row: number) => {
    const add = editing?.rowEnd(row);
    return add ? [<span key="add" className="panel-picture-add">{add}</span>] : [];
  };
  // A new row is drawn where it will land: after the user's rows, so above a native panel's last row of keys.
  const newRow: Line[] = editing?.newRow ? [{ key: "new-row", blank: false, keys: [<span key="add" className="panel-picture-slot" style={{ flexGrow: 1, flexBasis: 0 }}>{editing.newRow}</span>] }] : [];
  const lines: Line[] = [
    // While editing, the keys are scenery: the group's label already says they are there.
    ...(fixed ?? []).slice(0, -1).map((row, index): Line => ({ key: `fixed-${index}`, blank: false, keys: row.map((key, column) => <FixedTile key={column} cell={key} colours={colours} hidden={editing !== undefined} />) })),
    ...panel.rows.map((row, index): Line => ({ key: `row-${index}`, blank: false, row: index, keys: [...row.map((cell, column) => <span key={column} className="panel-picture-slot" style={{ flexGrow: cell.span, flexBasis: 0 }}>{userTile(cell, { row: index, cell: column })}</span>), ...rowEnd(index)] })),
    ...newRow,
    ...(fixed ?? []).slice(-1).map((row): Line => ({ key: "fixed-last", blank: false, keys: row.map((key, column) => <FixedTile key={column} cell={key} colours={colours} hidden={editing !== undefined} />) })),
  ];
  const columnEnds = columns.map((_, c) => editing?.columnEnd(c) || null);
  const longest = Math.max(0, ...columns.map((column, c) => column.length + (columnEnds[c] ? 1 : 0)));
  for (let index = lines.length; index < longest; index++) lines.push({ key: `below-${index}`, blank: true, keys: [] });
  const newColumn = editing?.newColumn || null;
  const slots = (index: number) => [
    ...columns.map((column, c) => column[index]
      ? <span key={`column-${c}`} className="panel-picture-slot" style={{ flexGrow: columnWidth(column), flexBasis: 0 }}>{userTile(column[index], { row: c, cell: index, column: true }, columnWidth(column))}</span>
      : index === column.length && columnEnds[c]
      ? <span key={`column-${c}`} className="panel-picture-slot" style={{ flexGrow: columnWidth(column), flexBasis: 0 }}>{columnEnds[c]}</span>
      : <span key={`column-${c}`} className="panel-picture-spacer" style={{ flexGrow: columnWidth(column), flexBasis: 0 }} />),
    // A new column starts beside the first line; the lines below keep its width clear.
    ...(newColumn ? [<span key="new-column" className="panel-picture-add">{index === 0 && newColumn}</span>] : []),
  ];
  const besideKeys = columns.length > 0 || newColumn !== null;
  const keys = (line: Line) => line.blank
    ? <span className="panel-picture-spacer" style={{ flexGrow: keysWeight, flexBasis: 0 }} />
    // With columns beside them, every line keeps its keys to the keys' width so the columns line up.
    : besideKeys ? <span className="panel-picture-keys" style={{ flexGrow: keysWeight, flexBasis: 0 }}>{line.keys}</span> : line.keys;
  const line = (entry: Line, index: number) => {
    const row = <div className="panel-picture-row" role={editing && entry.row !== undefined ? "group" : undefined} aria-label={editing && entry.row !== undefined ? `Row ${entry.row + 1}` : undefined}>{keys(entry)}{slots(index)}</div>;
    return editing
      ? <div className="panel-picture-line" key={entry.key}>{row}<div className="panel-picture-tools">{entry.row !== undefined && editing.rowTools(entry.row)}</div></div>
      : <div className="panel-picture-line" key={entry.key}>{row}</div>;
  };
  const accessible = editing
    ? { role: "group", "aria-label": `${panel.name} as it will scan: ${description}` }
    : { role: "img", "aria-label": `Picture of ${panel.name}: ${description}` };
  return <div className={`panel-picture ${className}`.trim()} {...accessible} style={{ background: colours.panel, borderColor: colours.border, color: colours.text }}>
    <div className="panel-picture-band" data-color={accent ?? undefined} style={{ background: accent ? colours.tiles[accent] : colours.band, borderColor: colours.bandBorder }}><span>{title}</span></div>
    <div className="panel-picture-rows">
      {lines.map(line)}
      {editing && columns.length > 0 && <div className="panel-column-toolbars">
        {columns.map((_, c) => <div key={c} className="panel-column-toolbar" role="group" aria-label={`Column ${c + 1} tools`}>
          <strong aria-hidden="true">Column {c + 1}</strong>
          <div className="panel-column-buttons">{editing.columnTools(c)}</div>
        </div>)}
      </div>}
    </div>
  </div>;
}

function FixedTile({ cell, colours, hidden }: { cell: PictureCell; colours: Palette; hidden?: boolean }) {
  return <span className="panel-picture-tile" data-fixed="true" aria-hidden={hidden || undefined} style={{ flexGrow: cell.weight, flexBasis: 0, background: colours.utility, borderColor: colours.border }}>{cell.label}</span>;
}

function PictureTile({ cell, colours, width, "data-column": column }: { cell: PanelCell; colours: Palette; width?: number; "data-column"?: number }) {
  const toolbar = toolbarAction(cell.action);
  return <span className="panel-picture-tile" data-toolbar={toolbar || undefined} data-color={cell.color} data-column={column} style={{ flexGrow: width ?? cell.span, flexBasis: 0, background: tileFace(cell, colours), borderColor: colours.border }}>{cell.label}</span>;
}
