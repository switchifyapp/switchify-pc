import type { AppSettings } from "../types";
import { PointerSection } from "./PointerSection";
import type { SettingsUpdate } from "./controls";

export function MouseSettingsView({ settings, onChange }: { settings: AppSettings; onChange: (next: AppSettings) => void }) {
  const update: SettingsUpdate = (key, value) => onChange({ ...settings, [key]: value });
  return <div className="view mouse-settings-view">
    <header className="page-header"><h1>Mouse</h1><p>Adjust pointer movement for Mouse scanning and Remote. Choose the scanning mode and panel timing under Scanning.</p></header>
    <PointerSection settings={settings} update={update} />
  </div>;
}
