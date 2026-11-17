import { useEffect, useId, useState } from "react";
import { Pause, Play } from "lucide-react";
import { Button } from "../ui/controls";
import { FeedbackMessage } from "../ui/patterns";
import { useReducedMotion } from "../help/Demonstration";
import { validSwitches, type ScanningController } from "../scanning/useScanning";
import type { ScanArea } from "../scanning/preferences";

/** A scan rate and line speed chosen together. Medium is the app's default pace. */
export const paces = [
  { id: "very-slow", label: "Very slow", intervalMs: 2500, speed: 0 },
  { id: "slow", label: "Slow", intervalMs: 1500, speed: 1 },
  { id: "medium", label: "Medium", intervalMs: 1000, speed: 2 },
  { id: "fast", label: "Fast", intervalMs: 700, speed: 3 },
] as const;

/** Line speed's own names, from slowest. */
const lineSpeeds = ["Very slow", "Slow", "Medium", "Fast", "Very fast"];
/** Scanners that can keep their own rate in Scanning, which a pace leaves alone. */
const areaNames: Record<ScanArea, string> = { menu: "the Home menu", point: "Point scanning", keyboard: "the keyboard", mouse: "the mouse panel" };

const previewItems = 6;
const seconds = (ms: number) => `${ms / 1000} s`;
const list = (names: string[]) => names.length < 2 ? names.join("") : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;

/** Choose how fast scanning moves, watching a highlight move at that pace first. */
export function PaceStep({ scanning, suspended = false }: { scanning: ScanningController; suspended?: boolean }) {
  const { blockIntervalMs, speed } = scanning.config;
  const current = paces.find(pace => pace.intervalMs === blockIntervalMs && pace.speed === speed);
  const supported = !!scanning.state?.supported;
  const ownRates = (Object.keys(areaNames) as ScanArea[]).filter(area => scanning.config.scanPreferences?.[area]?.intervalMs != null);
  const id = useId();
  const reduced = useReducedMotion();
  // Motion waits to be asked for when the person prefers less of it.
  const [playing, setPlaying] = useState(!reduced);
  useEffect(() => { if (reduced) setPlaying(false); }, [reduced]);
  const [item, setItem] = useState(0);
  const moving = playing && !suspended;
  useEffect(() => {
    if (!moving) return;
    const timer = window.setInterval(() => setItem(index => (index + 1) % previewItems), blockIntervalMs);
    return () => window.clearInterval(timer);
  }, [moving, blockIntervalMs]);
  const choose = (pace: typeof paces[number]) => {
    scanning.update("blockIntervalMs", pace.intervalMs);
    scanning.update("speed", pace.speed);
    setItem(0);
  };
  return <div className="setup-pace">
    <h3>How fast should scanning move?</h3>
    <p>Choose a pace you can keep up with. Slower is easier while you learn. You can change it at any time in Scanning.</p>
    {!supported && <p className="setting-note">{scanning.state?.message ?? "Loading scanning settings…"}</p>}
    <div className="setup-choices setup-pace-choices" role="group" aria-label="Scanning pace">
      {paces.map(pace => <Button key={pace.id} className="secondary" disabled={!supported} aria-label={pace.label} aria-describedby={`${id}-${pace.id}`} aria-pressed={current?.id === pace.id} onClick={() => choose(pace)}>
        <span><strong>{pace.label}</strong><small id={`${id}-${pace.id}`}>{seconds(pace.intervalMs)} on each item</small></span>
      </Button>)}
    </div>
    {/* Only an unsaved change can be retried; a failed load has nothing of the person's to save. */}
    {scanning.error && scanning.unsaved && <FeedbackMessage className="switch-error"><span>Your pace was not saved. {scanning.error}</span><Button type="button" className="secondary" disabled={!!scanning.pending || !validSwitches(scanning.config)} onClick={scanning.retry}>Retry save</Button></FeedbackMessage>}
    {scanning.error && !scanning.unsaved && <FeedbackMessage>{scanning.error}</FeedbackMessage>}
    {supported && !current && <p className="setting-note">Your scanning uses its own pace: {seconds(blockIntervalMs)} on each item, with Line speed {lineSpeeds[speed] ?? speed}. Choose one of these to change it.</p>}
    {ownRates.length > 0 && <p className="setting-note">{list(ownRates.map(area => areaNames[area])).replace(/^./, c => c.toUpperCase())} {ownRates.length === 1 ? "keeps its" : "keep their"} own scan rate, which a pace here does not change. Change {ownRates.length === 1 ? "it" : "them"} in Scanning.</p>}
    <div className="setup-pace-preview">
      <div className="setup-pace-preview-head">
        <h4>Preview</h4>
        <Button type="button" className="secondary" onClick={() => setPlaying(!playing)}>{playing ? <Pause size={16} /> : <Play size={16} />}{playing ? "Pause preview" : "Play preview"}</Button>
      </div>
      <ul aria-label={`Preview: the highlight moves every ${seconds(blockIntervalMs)}${moving ? "" : ", paused"}`}>
        {Array.from({ length: previewItems }, (_, index) => <li key={index} data-selected={index === item}>{index + 1}</li>)}
      </ul>
      <p>The pace also sets Line speed, how fast lines cross the screen in Point scanning, to the same level.</p>
    </div>
  </div>;
}
