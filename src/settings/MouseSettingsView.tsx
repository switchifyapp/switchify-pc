import { Button } from "../ui/controls";
import { validSwitches, type ScanningController } from "../scanning/useScanning";
import { OptionGroup } from "./controls";
import type { AppSettings } from "../types";
import { PointerSection } from "./PointerSection";
import type { SettingsUpdate } from "./controls";

export function MouseSettingsView({ settings, onChange, scanning }: { settings: AppSettings; onChange: (next: AppSettings) => void; scanning: ScanningController }) {
  const update: SettingsUpdate = (key, value) => onChange({ ...settings, [key]: value });
  return <div className="view mouse-settings-view">
    <header className="page-header"><div><h1>Mouse</h1><p>Pointer speed and repeat for Mouse scanning and Remote. Set panel scan timing under Scanning.</p></div></header>
    <PointerSection settings={settings} update={update}>
      <OptionGroup legend="Stop repeating" value={scanning.config.mouseRepeatStopEdge ?? "release"} disabled={!scanning.state?.supported || !!scanning.pending}
        options={[{ value: "press" as const, label: "On switch press" }, { value: "release" as const, label: "On switch release" }]}
        onChange={value => scanning.update("mouseRepeatStopEdge", value)}
        note={{ summary: "Applies to movement and scrolling in Mouse scanning, and to keys repeating on the scanned keyboard, including switches forwarded by Remote. On switch release keeps repeating while you hold the switch. The stop gesture ignores assigned switch actions." }} />
      <p role="status" className="setting-note">{scanning.pending ? "Saving scanning settings..." : scanning.unsaved ? "Scanning settings have unsaved changes." : "Saved automatically."}</p>
      {scanning.error && <p role="alert">{scanning.error}</p>}
      {scanning.error && scanning.unsaved && <Button type="button" className="secondary" disabled={!!scanning.pending || !validSwitches(scanning.config)} onClick={scanning.retry}>Retry save</Button>}
    </PointerSection>
  </div>;
}
