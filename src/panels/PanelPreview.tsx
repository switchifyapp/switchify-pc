import type { CustomPanel, PanelAction, PanelCell } from "../types";
import type { PanelTheme } from "../scanning/preferences";
import { lightPanels } from "../scanning/preferences";

/** The native panels' colours, so a preview looks like the panel will. */
const palettes = {
  dark: { panel: "#141820", band: "#1e2430", utility: "#222936", toolbar: "#191e28", border: "#3e4859", bandBorder: "#4a566c", text: "#ffffff" },
  light: { panel: "#f0f2f6", band: "#e2e6ed", utility: "#eceff4", toolbar: "#f4f6f9", border: "#c4cbd6", bandBorder: "#b0b9c8", text: "#141820" },
} as const;

/** Back and Close tiles are drawn like the panels' toolbar keys. */
export function toolbarAction(action: PanelAction) {
  return action.type === "back" || action.type === "close";
}

export function previewPalette(theme: PanelTheme | undefined) {
  return lightPanels(theme) ? palettes.light : palettes.dark;
}

/** A panel drawn as the scan will show it: a title band, then the rows of
 *  tiles sharing each row's width by span. Purely visual; nothing scans. */
export function PanelPreview({ panel, theme, title = "Switchify PC", className = "" }: { panel: CustomPanel; theme?: PanelTheme; title?: string; className?: string }) {
  const colours = previewPalette(theme);
  return <div className={`panel-preview ${className}`.trim()} role="img" aria-label={`Preview of ${panel.name}: ${panel.rows.length} ${panel.rows.length === 1 ? "row" : "rows"}`} style={{ background: colours.panel, borderColor: colours.border, color: colours.text }}>
    <div className="panel-preview-band" style={{ background: colours.band, borderColor: colours.bandBorder }}><span>{title}</span></div>
    <div className="panel-preview-rows">
      {panel.rows.map((row, index) => <div className="panel-preview-row" key={index}>
        {row.map((cell, column) => <PreviewTile key={column} cell={cell} colours={colours} />)}
      </div>)}
    </div>
  </div>;
}

function PreviewTile({ cell, colours }: { cell: PanelCell; colours: (typeof palettes)[keyof typeof palettes] }) {
  const toolbar = toolbarAction(cell.action);
  return <span className="panel-preview-tile" data-toolbar={toolbar || undefined} style={{ flexGrow: cell.span, flexBasis: 0, background: toolbar ? colours.toolbar : colours.utility, borderColor: colours.border }}>{cell.label}</span>;
}
