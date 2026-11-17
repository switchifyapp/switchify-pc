import { useEffect, useId, useState } from "react";
import { Pause, Play } from "lucide-react";
import { Button } from "../ui/controls";
import type { ScanningController } from "../scanning/useScanning";

/** A scan rate and line speed chosen together. Medium is the app's default pace. */
export const paces = [
  { id: "very-slow", label: "Very slow", intervalMs: 2500, speed: 0 },
  { id: "slow", label: "Slow", intervalMs: 1500, speed: 1 },
  { id: "medium", label: "Medium", intervalMs: 1000, speed: 2 },
  { id: "fast", label: "Fast", intervalMs: 700, speed: 3 },
] as const;

const previewItems = 6;
const seconds = (ms: number) => `${ms / 1000} s`;

/** Choose how fast scanning moves, watching a highlight move at that pace first. */
export function PaceStep({ scanning }: { scanning: ScanningController }) {
  const { blockIntervalMs, speed } = scanning.config;
  const current = paces.find(pace => pace.intervalMs === blockIntervalMs && pace.speed === speed);
  const id = useId();
  const [playing, setPlaying] = useState(true);
  const [item, setItem] = useState(0);
  useEffect(() => {
    if (!playing) return;
    const timer = window.setInterval(() => setItem(index => (index + 1) % previewItems), blockIntervalMs);
    return () => window.clearInterval(timer);
  }, [playing, blockIntervalMs]);
  const choose = (pace: typeof paces[number]) => {
    scanning.update("blockIntervalMs", pace.intervalMs);
    scanning.update("speed", pace.speed);
    setItem(0);
  };
  return <div className="setup-pace">
    <h3>How fast should scanning move?</h3>
    <p>Choose a pace you can keep up with. Slower is easier while you learn. You can change it at any time in Scanning.</p>
    <div className="setup-choices setup-pace-choices" role="group" aria-label="Scanning pace">
      {paces.map(pace => <Button key={pace.id} className="secondary" disabled={!scanning.state?.supported} aria-label={pace.label} aria-describedby={`${id}-${pace.id}`} aria-pressed={current?.id === pace.id} onClick={() => choose(pace)}>
        <span><strong>{pace.label}</strong><small id={`${id}-${pace.id}`}>{seconds(pace.intervalMs)} on each item</small></span>
      </Button>)}
    </div>
    {!current && <p className="setting-note">Your scanning uses its own pace: {seconds(blockIntervalMs)} on each item. Choose one of these to change it.</p>}
    <div className="setup-pace-preview">
      <div className="setup-pace-preview-head">
        <h4>Preview</h4>
        <Button type="button" className="secondary" onClick={() => setPlaying(!playing)}>{playing ? <Pause size={16} /> : <Play size={16} />}{playing ? "Pause preview" : "Play preview"}</Button>
      </div>
      <ul aria-label={`Preview: the highlight moves every ${seconds(blockIntervalMs)}`}>
        {Array.from({ length: previewItems }, (_, index) => <li key={index} data-selected={index === item}>{index + 1}</li>)}
      </ul>
      <p>The pace also sets Line speed, how fast lines cross the screen in Point scanning, to the same level.</p>
    </div>
  </div>;
}
