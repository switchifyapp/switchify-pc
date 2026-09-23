import type { AppSettings } from "../types";
import { PointerSection } from "./PointerSection";
import type { SettingsUpdate } from "./controls";

export function MouseSettingsView({ settings, onChange }: { settings: AppSettings; onChange: (next: AppSettings) => void }) {
  const update: SettingsUpdate = (key, value) => onChange({ ...settings, [key]: value });
  return <div className="view mouse-settings-view">
    <header className="page-header"><div><h1>Mouse</h1><p>Pointer speed and repeat for Mouse scanning and Remote. Set panel scan timing under Scanning.</p></div></header>
    <PointerSection settings={settings} update={update} />
  </div>;
}
