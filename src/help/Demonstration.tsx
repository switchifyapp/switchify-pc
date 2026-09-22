import { Button, MoreOptions } from "../ui/controls";
import { createContext, useContext, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { demonstration, type DemonstrationKind, type DemoPlatform } from "./demonstrations";
import { PrototypeArtwork } from "./PrototypeArtwork";
import { TeachingArtwork } from "./TeachingArtwork";
import "./demonstrations.css";

const DemoContext = createContext<{ suspended: boolean; platform: DemoPlatform }>({ suspended: false, platform: "windows" });

export function DemonstrationProvider({ suspended, platform, children }: { suspended: boolean; platform: DemoPlatform; children: ReactNode }) {
  return <DemoContext.Provider value={{ suspended, platform }}>{children}</DemoContext.Provider>;
}

function useReducedMotion() {
  const [reduced, setReduced] = useState(() => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false);
  useEffect(() => {
    const query = window.matchMedia?.("(prefers-reduced-motion: reduce)");
    if (!query) return;
    const update = () => setReduced(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return reduced;
}

function Player({ kind }: { kind: DemonstrationKind }) {
  const { suspended, platform } = useContext(DemoContext);
  const { title, steps, captions } = demonstration(kind, platform);
  const reduced = useReducedMotion();
  const [step, setStep] = useState(0);
  const [progress, setProgress] = useState(0);
  const elapsed = useRef(0);
  const [playing, setPlaying] = useState(!reduced);
  const [visible, setVisible] = useState(typeof IntersectionObserver === "undefined");
  const [pageVisible, setPageVisible] = useState(!document.hidden);
  const root = useRef<HTMLDivElement>(null);
  const id = useId();
  useEffect(() => {
    if (!root.current || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting), { threshold: 0 });
    observer.observe(root.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const changed = () => setPageVisible(!document.hidden);
    document.addEventListener("visibilitychange", changed);
    return () => document.removeEventListener("visibilitychange", changed);
  }, []);
  useEffect(() => { if (reduced || suspended) setPlaying(false); }, [reduced, suspended]);
  useEffect(() => {
    if (!playing || reduced || suspended || !visible || !pageVisible) return;
    let frame = 0;
    let previous: number | undefined;
    const tick = (now: number) => {
      if (previous !== undefined) elapsed.current += Math.min(now - previous, 100);
      previous = now;
      setProgress(Math.min(1, elapsed.current / 3000));
      if (elapsed.current >= 3000) {
        if (step === steps.length - 1) setPlaying(false);
        else { elapsed.current = 0; setProgress(0); setStep(step + 1); }
      } else frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [playing, reduced, suspended, visible, pageVisible, step, steps.length]);

  return <div className="teaching-demo" ref={root} data-reduced={reduced} data-moving={playing && !suspended && visible && pageVisible && !reduced} data-step={step}>
    <p id={`${id}-example`} className="setting-note">Illustrated example — this does not operate your switches or change settings.</p>
    <div className="teaching-art" aria-describedby={`${id}-example`}>
      {kind === "grid" || kind === "line" || kind === "hold" ? <TeachingArtwork kind={kind} step={step} progress={reduced ? 1 : progress} /> : <PrototypeArtwork kind={kind} step={step} platform={platform} />}
    </div>
    <p className="teaching-caption">{step + 1} / {steps.length}: {captions[step]}</p>
    <div className="teaching-controls">
      {!reduced && <><Button type="button" className="secondary" disabled={suspended || (!playing && step === steps.length - 1 && progress >= 1)} onClick={() => setPlaying(!playing)} aria-label={`${playing ? "Pause" : "Play"} ${title}`}>{playing ? "Pause" : "Play"}</Button><Button type="button" className="secondary" disabled={suspended} onClick={() => { elapsed.current = 0; setProgress(0); setStep(0); setPlaying(true); }} aria-label={`Replay ${title}`}>Replay</Button></>}
      {reduced && <><Button type="button" className="secondary" disabled={step === 0 || suspended} onClick={() => setStep(step - 1)} aria-label={`Previous illustration: ${title}`}>Previous illustration</Button><Button type="button" className="secondary" disabled={step === steps.length - 1 || suspended} onClick={() => setStep(step + 1)} aria-label={`Next illustration: ${title}`}>Next illustration</Button></>}
    </div>
    {reduced && <p className="setting-note">Reduced motion: use the buttons to view each still illustration.</p>}
    <MoreOptions label="Read steps" accessibleLabel={`Read steps: ${title}`}><ol className="teaching-steps" aria-label={`${title} instructions`}>{steps.map((text, index) => <li key={text} aria-current={index === step ? "step" : undefined}>{text}</li>)}</ol></MoreOptions>
  </div>;
}

export function Demonstration({ kind }: { kind: DemonstrationKind }) {
  const { platform } = useContext(DemoContext);
  const [open, setOpen] = useState(false);
  const id = useId();
  if (kind === "access" && platform !== "macos") return null;
  const { title } = demonstration(kind, platform);
  return <div className="teaching-disclosure">
    <Button type="button" className="disclosure" aria-expanded={open} aria-controls={open ? id : undefined} aria-label={`${open ? "Hide" : "Show me how"}: ${title}`} onClick={() => setOpen(!open)}><span>{title}</span><span>{open ? "Hide" : "Show me how"}</span></Button>
    {open && <div id={id}><Player kind={kind} /></div>}
  </div>;
}

export function SwitchDemonstrations() {
  return <><Demonstration kind="jack" /><Demonstration kind="usb" /><Demonstration kind="hold" /></>;
}
