import { AppWindow, ArrowLeft, Crosshair, Keyboard, LayoutGrid, Mouse, PanelTop, Pencil, Play, SlidersHorizontal, X, Zap, ZoomIn, type LucideIcon } from "lucide-react";
import type { PanelAction } from "../types";

/** Mirrors the native Home renderer. Labels never decide an icon. */
export function homePresentation(action: PanelAction): { Icon: LucideIcon; navigation: boolean } {
  if (action.type === "close") return { Icon: X, navigation: true };
  if (action.type === "back") return { Icon: ArrowLeft, navigation: true };
  if (action.type === "myPanels") return { Icon: LayoutGrid, navigation: true };
  if (action.type === "openCustomPanel") return { Icon: LayoutGrid, navigation: false };
  if (action.type === "openPanel") {
    const panel = action.panel;
    if (panel === "point") return { Icon: Crosshair, navigation: false };
    if (panel === "mouse") return { Icon: Mouse, navigation: false };
    if (panel === "keyboard") return { Icon: Keyboard, navigation: false };
    if (panel === "switchify") return { Icon: SlidersHorizontal, navigation: true };
    if (typeof panel === "object") {
      const Icon = ({ windows: AppWindow, editing: Pencil, browser: PanelTop, tabs: PanelTop, media: Play, zoom: ZoomIn, panels: LayoutGrid, custom: LayoutGrid } as Record<string, LucideIcon>)[panel.menu] ?? Zap;
      return { Icon, navigation: false };
    }
  }
  return { Icon: Zap, navigation: false };
}

export function HomeTileContent({ action, label }: { action: PanelAction; label: string }) {
  const { Icon } = homePresentation(action);
  return <><Icon size={24} aria-hidden="true" /><span>{label || <em>No label</em>}</span></>;
}
