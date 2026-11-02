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
export function PanelPicture({ panel, fixed, theme, title = "Switchify PC", className = "" }: { panel: CustomPanel; fixed?: PictureCell[][]; theme?: PanelTheme; title?: string; className?: string }) {
  const colours = picturePalette(theme);
  // The name says what is on the panel, row by row, since the tiles are decoration here.
  const rows = panel.rows.map((row) => row.map((cell) => cell.label.trim() || "unlabelled").join(", ")).join("; ");
  const description = fixed ? (rows ? `its keys, then ${rows}` : "its keys only") : rows;
  const fixedRow = (row: PictureCell[], index: number) => <div className="panel-picture-row" key={`fixed-${index}`}>
    {row.map((key, column) => <span key={column} className="panel-picture-tile" data-fixed="true" style={{ flexGrow: key.weight, flexBasis: 0, background: colours.utility, borderColor: colours.border }}>{key.label}</span>)}
  </div>;
  return <div className={`panel-picture ${className}`.trim()} role="img" aria-label={`Picture of ${panel.name}: ${description}`} style={{ background: colours.panel, borderColor: colours.border, color: colours.text }}>
    <div className="panel-picture-band" style={{ background: colours.band, borderColor: colours.bandBorder }}><span>{title}</span></div>
    <div className="panel-picture-rows">
      {fixed?.slice(0, -1).map(fixedRow)}
      {panel.rows.map((row, index) => <div className="panel-picture-row" key={index}>
        {row.map((cell, column) => <PictureTile key={column} cell={cell} colours={colours} />)}
      </div>)}
      {fixed?.slice(-1).map((row) => fixedRow(row, fixed.length - 1))}
    </div>
  </div>;
}

function PictureTile({ cell, colours }: { cell: PanelCell; colours: (typeof palettes)[keyof typeof palettes] }) {
  const toolbar = toolbarAction(cell.action);
  return <span className="panel-picture-tile" data-toolbar={toolbar || undefined} style={{ flexGrow: cell.span, flexBasis: 0, background: toolbar ? colours.toolbar : colours.utility, borderColor: colours.border }}>{cell.label}</span>;
}
