import { useEffect, useRef, useState } from "react";
import { CircleCheck } from "lucide-react";
import { Button } from "../ui/controls";
import { FeedbackMessage } from "../ui/patterns";
import { Demonstration } from "../help/Demonstration";
import { practiceAvailable, practiceInstructions, usePractice } from "../scanning/SwitchPractice";

/** The block the guide asks for: far enough along that the highlight has to move. */
export const goalBlock = "Block 3";

function AfterSetup({ escapeSeconds }: { escapeSeconds: number }) {
  return <div className="setup-after">
    <h3>After setup</h3>
    <ul>
      <li>Press and release <strong>Select</strong> to open Home.</li>
      <li>From Home, <strong>Point</strong> picks a place on the screen, <strong>Mouse</strong> moves the pointer, and <strong>Keyboard</strong> types.</li>
      <li>To stop scanning, press <strong>Escape</strong> or hold any switch for {escapeSeconds} seconds.</li>
    </ul>
    <Demonstration kind="grid" /><Demonstration kind="line" /><Demonstration kind="hold" />
  </div>;
}

/** Use a switch for real before setup ends, with practice that cannot click or type. */
export function TryItStep({ hasSwitch, phone, automatic, escapeSeconds, suspended }: { hasSwitch: boolean; phone: boolean; automatic: boolean; escapeSeconds: number; suspended: boolean }) {
  const { open, view, error, pending, begin, close } = usePractice();
  const reached = view?.lastChoice === goalBlock;
  // Practice clears its view when it ends; the success stays.
  const [succeeded, setSucceeded] = useState(false);
  useEffect(() => { if (reached) setSucceeded(true); }, [reached]);
  // A pairing request covers the guide, so practice must not keep the switches meanwhile.
  useEffect(() => { if (suspended && open) void close(); }, [suspended, open]);
  // Focus follows the control that replaces the one just used, so nobody is left on the page body.
  const startRef = useRef<HTMLButtonElement>(null);
  const stopRef = useRef<HTMLButtonElement>(null);
  const [ended, setEnded] = useState(false);
  const wasOpen = useRef(false);
  useEffect(() => {
    if (open) { setEnded(false); stopRef.current?.focus(); }
    else if (wasOpen.current) { setEnded(true); if (!suspended) startRef.current?.focus(); }
    wasOpen.current = open;
  }, [open]);
  if (phone) return <div className="setup-try">
    <h3>Try it from your phone</h3>
    <p>On your phone, open Switchify, choose <strong>PC control</strong>, and pick this computer. Its Mouse, Typing and Window controls then work this computer. Scanning on this computer pauses while the phone is connected.</p>
    <AfterSetup escapeSeconds={escapeSeconds} />
  </div>;
  const missed = !!view?.lastChoice && !reached;
  const highlighted = view?.blocks.find(block => block.selected)?.label;
  return <div className="setup-try">
    <h3>Choose {goalBlock} with your switch</h3>
    {!hasSwitch ? <p>Add a switch first to try it here. You can also practise later from <strong>Switches</strong>.</p> : <>
      <p>{practiceInstructions(view?.automatic ?? automatic)} Nothing here clicks or types.</p>
      <p>To stop, choose <strong>Stop practice</strong>, press <strong>Escape</strong>, or hold any switch for {escapeSeconds} seconds. Practice also ends by itself after two minutes.</p>
      {!open && <Button ref={startRef} type="button" className="primary" disabled={pending || suspended || !practiceAvailable()} onClick={() => void begin(false)}>Start practice</Button>}
      {open && <div className="setup-try-practice">
        <ul className="switch-practice-blocks" aria-label="Practice blocks">
          {(view?.blocks ?? []).map(block => <li key={block.id} data-selected={block.selected} aria-current={block.selected || undefined} data-goal={block.label === goalBlock || undefined}>
            {block.label}{block.label === goalBlock && <small className="setup-try-goal">Goal</small>}
          </li>)}
        </ul>
        <p aria-live="polite">Highlighted: {highlighted ?? "none"}</p>
        <p role="status" aria-live="polite">{reached ? `You chose ${goalBlock}. Your switch is working.` : view?.stopped ? view.message : missed ? `You chose ${view?.lastChoice}. Press Select to try again for ${goalBlock}.` : view?.message ?? "Starting practice…"}</p>
        <Button ref={stopRef} type="button" className="secondary" disabled={pending} onClick={() => void close()}>Stop practice</Button>
      </div>}
      {!open && <p className={succeeded ? "setup-try-done" : undefined} role="status">{succeeded ? <><CircleCheck size={20} aria-hidden="true" />You chose {goalBlock}. Your switch is working.</> : ended ? "Practice ended. Choose Start practice to try again." : null}</p>}
      {error && <FeedbackMessage>{error}</FeedbackMessage>}
    </>}
    <AfterSetup escapeSeconds={escapeSeconds} />
  </div>;
}
