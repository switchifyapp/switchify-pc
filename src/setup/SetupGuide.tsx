import { useEffect, useId, useRef, useState } from "react";
import { Accessibility, Bluetooth, CircleCheck } from "lucide-react";
import { Dialog } from "../ui/Dialog";
import { FeedbackMessage } from "../ui/patterns";
import { Button, StatusIcon } from "../ui/controls";
import { PageIcon } from "../ui/PageIcon";
import { needsInputAccess } from "../inputAccess";
import { Demonstration } from "../help/Demonstration";
import { SwitchesSection } from "../settings/SwitchesSection";
import type { SwitchController } from "../scanning/useSwitches";
import type { ScanningController } from "../scanning/useScanning";
import { PaceStep } from "./PaceStep";
import { TryItStep } from "./TryItStep";
import { areaOptions } from "../scanning/preferences";
import { bluetoothDescriptions, bluetoothLabels, bluetoothUnavailable, MobileInstallCards } from "../mobile";
import type { AppState } from "../types";
import type { Page } from "../find/index";

export function AccessibilityCopy({ state, detailed = false }: { state: AppState; detailed?: boolean }) {
  if (state.accessibility === "granted") return <p>Ready</p>;
  if (state.accessibility === "unavailable") return <p>Unavailable on this system</p>;
  if (!detailed || state.capabilities.platform !== "macos") return <p>Permission required</p>;
  return <>
    <p>Enable “Switchify PC” in Accessibility, then return to the app. Status updates automatically.</p>
    <p className="permission-recovery">If it is already enabled but access is still required, select the stale row, click Remove, return to Switchify, reopen Accessibility Settings, and enable the newly added entry.</p>
  </>;
}

/** How the person reaches this computer: a switch plugged into it, or Switchify on a phone or tablet. */
type Connection = "computer" | "phone";
type Step = "access" | "connect" | "switch" | "phone" | "pace" | "try" | "choices";
const titles: Record<Step, string> = {
  access: "Input access", connect: "How will you connect?", switch: "Add your switch", phone: "Connect your phone", pace: "Scanning pace",
  try: "Try it", choices: "Last choices",
};

export function SetupGuide({ state, switches, scanning, suspended, busy, error, skip, finish, close, accessibility }: {
  state: AppState;
  busy: boolean;
  error: string | null;
  skip: () => Promise<void>;
  /** Saves the last choices; true once setup is complete. */
  finish: (startWithSystem: boolean, shareDiagnostics: boolean) => Promise<boolean>;
  /** Closes the guide, opening `page` if given. */
  close: (page?: Page) => void;
  accessibility: () => Promise<void>;
  switches: SwitchController;
  scanning: ScanningController;
  suspended: boolean;
}) {
  const inputAccessRequired = needsInputAccess(state);
  const phoneUnavailable = bluetoothUnavailable(state.bluetooth);
  // Reopened after setup, the guide starts from what is already there.
  const [chosen, setConnection] = useState<Connection | null>(() =>
    switches.state?.settings.bindings.length ? "computer" : state.pairedDevices.length && !phoneUnavailable ? "phone" : null);
  // A phone choice no longer counts once Bluetooth cannot be used.
  const connection = chosen === "phone" && phoneUnavailable ? null : chosen;
  const savedSwitches = !!switches.state?.settings.bindings.length;
  useEffect(() => { if (savedSwitches) setConnection(current => current ?? "computer"); }, [savedSwitches]);
  const steps: Step[] = [...(inputAccessRequired ? ["access" as const] : []), "connect", ...(connection === "phone" ? ["phone" as const] : ["switch" as const, "pace" as const]), "try", "choices"];
  const [requestedStep, setStep] = useState<Step>(() => inputAccessRequired ? "access" : "connect");
  // Permission can arrive while this guide is open. Move straight on without
  // losing saved choices.
  // A step can also leave the list, such as the phone step when Bluetooth stops
  // working; the guide then asks again how the person will connect.
  const step = requestedStep === "access" && !inputAccessRequired ? "connect" : steps.includes(requestedStep) ? requestedStep : "connect";
  const index = steps.indexOf(step);
  useEffect(() => {
    if (!inputAccessRequired && requestedStep === "access") setStep("connect");
  }, [inputAccessRequired, requestedStep]);
  const [startupChoice, setStartupChoice] = useState<boolean | null>(state.setup.completed ? state.settings.startWithSystem : null);
  const [diagnosticsChoice, setDiagnosticsChoice] = useState<boolean | null>(state.setup.completed && state.telemetry.consent !== "undecided" ? state.telemetry.consent === "enabled" : null);
  const dialogRef = useRef<HTMLElement>(null);
  const [switchDraft, setSwitchDraft] = useState(false);
  const capturing = switches.capturing || !!switches.state?.capture.active;
  // Switch saving holds the guide only on the switch step, where it can be retried;
  // elsewhere, an earlier failed save must not trap someone in the guide.
  const switchBusy = step === "switch" && (capturing || !!switches.pending || switches.unsaved || switchDraft);
  const paired = state.pairedDevices.length > 0;
  const hasSwitch = !!switches.state?.settings.bindings.length;
  const canContinue = step === "connect" ? connection !== null
    : step === "switch" ? !switchBusy && hasSwitch
      : step === "phone" ? paired
        : step === "choices" ? startupChoice !== null && diagnosticsChoice !== null : true;
  const last = index === steps.length - 1;
  // Setup is saved before the ready screen, so closing it any way keeps the choices.
  const [ready, setReady] = useState(false);
  const next = () => last ? void finish(startupChoice!, diagnosticsChoice!).then(done => { if (done) setReady(true); }) : setStep(steps[index + 1]);
  const guidanceId = useId();
  const navigationGuidance = !switchBusy ? null : capturing
    ? "Finish capturing a switch or cancel capture before continuing."
    : switches.pending
      ? "Wait for your switch changes to finish saving before continuing."
      : switches.unsaved
        ? "Your switch changes have not saved. Retry saving before continuing."
        : "Save or cancel the new switch before continuing.";
  const stepGuidance = busy
            ? "Please wait while setup saves your changes."
            : step === "connect" && connection === null
              ? "Choose how you will connect to enable Next."
              : step === "switch" && !hasSwitch
                ? "Add and save a switch to enable Next, or choose Set up switches later."
                : step === "phone" && !paired
                  ? "Pair your phone or tablet to enable Next, or choose Pair later."
                  : step === "choices" && startupChoice === null && diagnosticsChoice === null
                    ? "Choose how Switchify starts and whether to share diagnostics to enable Finish."
                    : step === "choices" && startupChoice === null
                      ? "Choose Start with system or Start manually to enable Finish."
                      : step === "choices" && diagnosticsChoice === null
                        ? "Choose whether to share diagnostics to enable Finish."
                        : null;

  const guidance = navigationGuidance ?? stepGuidance;

  useEffect(() => { if (!suspended && !capturing) dialogRef.current?.focus(); }, [step, ready, suspended]);

  if (ready) return <Dialog dialogRef={dialogRef} className="setup-dialog" backdropClassName="setup-backdrop" suspended={suspended} onEscape={() => close()} aria-labelledby="setup-title" aria-describedby={`${guidanceId}-ready`}>
    <header><div><span>Setup guide</span><h2 id="setup-title">You’re ready</h2></div></header>
    <div className="setup-progress" aria-hidden="true" style={{ gridTemplateColumns: `repeat(${steps.length}, 1fr)` }}>{steps.map(id => <span key={id} data-active />)}</div>
    <div className="setup-content setup-ready">
      <p className="setup-ready-lead" id={`${guidanceId}-ready`}><CircleCheck size={28} aria-hidden="true" />{connection === "phone"
        ? paired ? "Open PC control in Switchify on your phone to start." : "Pair your phone from Mobile when you are ready, then open PC control in Switchify."
        : hasSwitch ? "Press and release Select to open Home." : "Add a switch in Switches when you are ready, then press Select to open Home."}</p>
      <h3 id={`${guidanceId}-next`}>Next steps</h3>
      <div className="task-list" role="group" aria-labelledby={`${guidanceId}-next`}>
        <article><PageIcon page="scanning" size={24} /><div><h4>Adjust scanning</h4><p>Speed, colour, manual or automatic, and what Select opens.</p></div><Button className="secondary" onClick={() => close("scanning")}>Open Scanning</Button></article>
        <article><PageIcon page="panels" size={24} /><div><h4>Make a panel</h4><p>Tiles that type a phrase, press keys or run an action.</p></div><Button className="secondary" onClick={() => close("panels")}>Open Panels</Button></article>
        <article><PageIcon page="support" size={24} /><div><h4>Get help</h4><p>Fix common problems or open this guide again.</p></div><Button className="secondary" onClick={() => close("support")}>Open Help</Button></article>
      </div>
    </div>
    <footer><span /><Button className="primary" onClick={() => close()}>Start using Switchify PC</Button></footer>
  </Dialog>;

  return <Dialog dialogRef={dialogRef} className="setup-dialog" backdropClassName="setup-backdrop" suspended={suspended} active={!capturing} data-switch-step={step === "switch"} aria-labelledby="setup-title">
    <header><div><span>Setup guide</span><h2 id="setup-title">{titles[step]}</h2></div><strong aria-label={`Step ${index + 1} of ${steps.length}`}>{index + 1} / {steps.length}</strong></header>
    <div className="setup-progress" aria-hidden="true" style={{ gridTemplateColumns: `repeat(${steps.length}, 1fr)` }}>{steps.map((id, i) => <span key={id} data-active={i <= index} />)}</div>
    {error && <FeedbackMessage>{error}</FeedbackMessage>}
    <div className="setup-content">
      {step === "access" && <div className="setup-statuses">
        {needsInputAccess(state) && <article><StatusIcon ok={false}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} detailed /><Demonstration kind="access" /></div><Button className="secondary" disabled={busy} onClick={() => void accessibility()}>Open Accessibility Settings</Button></article>}
      </div>}
      {step === "connect" && <div>
        <h3>Choose how you will use Switchify PC</h3>
        <p>You can change this later. Switches on this computer and a paired phone can both be set up.</p>
        <div className="setup-choices setup-connection" role="group" aria-label="How you will connect">
          <Button className="secondary" aria-pressed={connection === "computer"} aria-labelledby={`${guidanceId}-computer-name`} aria-describedby={`${guidanceId}-computer`} onClick={() => setConnection("computer")}>
            <PageIcon page="switches" size={28} /><span><strong id={`${guidanceId}-computer-name`}>A switch on this computer</strong><small id={`${guidanceId}-computer`}>Through a switch interface or a USB switch that sends a key.</small></span>
          </Button>
          <Button className="secondary" disabled={phoneUnavailable} aria-pressed={connection === "phone"} aria-labelledby={`${guidanceId}-phone-name`} aria-describedby={`${guidanceId}-phone`} onClick={() => setConnection("phone")}>
            <PageIcon page="mobile" size={28} /><span><strong id={`${guidanceId}-phone-name`}>Switchify on my Android phone or tablet</strong><small id={`${guidanceId}-phone`}>{phoneUnavailable ? `Not available: ${bluetoothDescriptions[state.bluetooth]}` : "Pair over Bluetooth, then control this computer from the phone."}</small></span>
          </Button>
        </div>
      </div>}
      {step === "switch" && <SwitchesSection compact mobileConnected={state.bluetooth === "connected"} platform={state.capabilities.platform} controller={switches} onDraftChange={setSwitchDraft} suspended={suspended} />}
      {step === "phone" && <div className="setup-phone">
        <div className="setup-phone-status" role="status"><StatusIcon ok={paired || undefined}><Bluetooth size={19} /></StatusIcon><div>
          <h3>{paired ? `Paired with ${state.pairedDevices[0].deviceName}` : bluetoothLabels[state.bluetooth]}</h3>
          <p>{paired ? "You can pair more devices later from Mobile." : bluetoothDescriptions[state.bluetooth]}</p>
        </div></div>
        <MobileInstallCards attributes={false} nested />
      </div>}
      {step === "pace" && <PaceStep scanning={scanning} suspended={suspended} />}
      {step === "try" && <TryItStep hasSwitch={hasSwitch} phone={connection === "phone"} automatic={areaOptions(scanning.config, "menu").automatic} escapeSeconds={switches.state ? switches.state.escapeHoldMs / 1000 : 4} suspended={suspended} />}
      {step === "choices" && <div className="setup-last-choices">
        <section aria-labelledby={`${guidanceId}-startup`}><h3 id={`${guidanceId}-startup`}>When should Switchify PC start?</h3><p>It can start quietly when you sign in, ready for your switches.</p><div className="setup-choices" role="group" aria-label="Start with system choice"><Button className="secondary" aria-pressed={startupChoice === true} onClick={() => setStartupChoice(true)}>Start with system</Button><Button className="secondary" aria-pressed={startupChoice === false} onClick={() => setStartupChoice(false)}>Start manually</Button></div><Demonstration kind="startup" /></section>
        <section aria-labelledby={`${guidanceId}-diagnostics`}><h3 id={`${guidanceId}-diagnostics`}>Share anonymous diagnostics?</h3><p>Optional anonymous app health, internal error reports and sanitized errors, sent to Sentry, help improve Switchify. Typed text, commands, pairing secrets, device names, and full paths are never included.</p><div className="setup-choices" role="group" aria-label="Anonymous diagnostics choice"><Button className="secondary" disabled={!state.telemetry.available} aria-pressed={diagnosticsChoice === true} onClick={() => setDiagnosticsChoice(true)}>Share diagnostics</Button><Button className="secondary" aria-pressed={diagnosticsChoice === false} onClick={() => setDiagnosticsChoice(false)}>Don’t share</Button></div><a className="setup-privacy" href="https://www.theswitchify.app/privacy" target="_blank" rel="noreferrer">Privacy policy</a></section>
      </div>}
    </div>
    <p id={guidanceId} className="setup-navigation-guidance" role="status">{guidance}</p>
    <footer>
      <Button className="text-button" disabled={busy || switchBusy} onClick={() => void skip()}>Skip setup for now</Button>
      {step === "switch" && !hasSwitch && <Button className="secondary" disabled={busy || switchBusy} onClick={next}>Set up switches later</Button>}
      {step === "phone" && !paired && <Button className="secondary" disabled={busy} onClick={next}>Pair later</Button>}
      <span />
      <Button className="secondary" disabled={busy || switchBusy || index === 0} onClick={() => setStep(steps[index - 1])}>Back</Button>
      <Button className="primary" aria-describedby={guidance ? guidanceId : undefined} disabled={busy || switchBusy || !canContinue} onClick={next}>{last ? "Finish" : "Next"}</Button>
    </footer>
  </Dialog>;
}
