import type { DemonstrationKind } from "./demonstrations";

function Switch({ down, x, y }: { down: boolean; x: number; y: number }) {
  return <g transform={`translate(${x} ${y})`}><circle r="35" fill="#2a2a32" /><circle r="24" cy={down ? 6 : -3} fill="var(--brand)" /></g>;
}

export function TeachingArtwork({ kind, step, progress }: { kind: DemonstrationKind; step: number; progress: number }) {
  if (kind === "hold") return <svg viewBox="0 0 720 405" aria-hidden="true" focusable="false">
    <rect width="720" height="405" rx="10" fill="var(--surface)" />
    <text x="40" y="45">Example switch actions</text>
    <Switch x={120} y={170} down={step === 0 || step === 2 || step === 3} />
    <text x="45" y="255">{step === 0 || step === 2 || step === 3 ? "Held: movement frozen" : "Released"}</text>
    {["Select", "Pause / resume", "Stop scanning"].map((label, i) => <g key={label} transform={`translate(355 ${80 + i * 85})`}>
      <rect width="310" height="60" rx="8" fill={(step < 2 ? i === 0 : step === 2 ? i === 1 : i === 2) ? "var(--brand-container)" : "var(--surface)"} stroke="var(--border)" strokeWidth="2" />
      <text x="18" y="36">{label}</text>
    </g>)}
    <text x="40" y="365">{step === 1 ? "Release → Select runs" : step === 4 ? "Release → Stop scanning runs" : "Actions run on release"}</text>
  </svg>;

  const grid = kind === "grid";
  const phase = grid ? step : step === 0 ? 0 : step + 2;
  const selected = progress < 0.75 ? Math.min(2, Math.floor(progress * 4)) : 1;
  const column = phase === 2 ? selected : 1;
  const row = phase === 1 ? selected : 1;
  const travel = Math.min(1, progress / 0.8);
  const lineX = phase === 3 ? (grid ? 187 : 40) + travel * (grid ? 103 : 250) : 290;
  const lineY = phase === 4 ? (grid ? 143 : 50) + travel * (grid ? 37 : 130) : 180;
  return <svg viewBox="0 0 720 405" aria-hidden="true" focusable="false">
    <rect width="720" height="405" rx="10" fill="var(--surface)" />
    <rect x="40" y="50" width="440" height="280" rx="6" fill="var(--surface-variant, #f3f2f6)" stroke="var(--border)" />
    {grid && Array.from({ length: 9 }, (_, i) => <rect key={i} x={40 + (i % 3) * 146.7} y={50 + Math.floor(i / 3) * 93.3} width="146.7" height="93.3" fill={(phase === 1 && Math.floor(i / 3) === row) || (phase >= 2 && i === 3 + column) ? "var(--brand-container)" : "transparent"} stroke="var(--border)" />)}
    {phase >= 3 && <line x1={lineX} x2={lineX} y1={grid ? 143 : 50} y2={grid ? 237 : 330} stroke="var(--brand)" strokeWidth="3" />}
    {phase >= 4 && <line x1={grid ? 187 : 40} x2={grid ? 333 : 480} y1={lineY} y2={lineY} stroke="var(--brand)" strokeWidth="3" />}
    {phase >= 5 && <circle cx={lineX} cy={lineY} r="8" fill="var(--brand)" />}
    {phase === 5 && <g><rect x="490" y="105" width="210" height="150" rx="10" fill="var(--brand-container)" /><text x="505" y="138">Auto select on</text><text x="505" y="173">Countdown → click</text><text x="505" y="208">Switch → menu</text></g>}
    {phase === 6 && <g><rect x="490" y="80" width="210" height="205" rx="10" fill="var(--surface)" stroke="var(--brand)" />{["Action menu", "Left click", "Right click", "Scroll"].map((label, i) => <text key={label} x="505" y={112 + i * 45}>{label}</text>)}</g>}
    <text x="40" y="380">{["Select starts scanning", "Choose a row", "Choose a cell", "Choose horizontal position", "Choose vertical position", "Auto select enabled", "Auto select disabled"][phase]}</text>
  </svg>;
}
