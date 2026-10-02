import { Button } from "../ui/controls";
import { Demonstration } from "../help/Demonstration";
import { type ScanningController, validSwitches, mouseRepeatStopInstruction } from "../scanning/useScanning";
import { ScannerPreferences } from "./ScannerPreferences";

const phases = {
  keyboard: "Choose a keyboard row, then a key",
  keyboardSuspended: "Select to resume the keyboard",
  keyboardOpening: "Opening the keyboard",
  mouse: "Choose a mouse control",
  mouseSuspended: "Select to resume the mouse",
  mouseMoving: "Moving pointer",
  mouseScrolling: "Scrolling",
  idle: "Ready to begin",
  autoSelecting: "Waiting to click. Press a switch for the action menu",
  menu: "Choose an action at the selected point",
  menuSuspended: "Select to resume the action menu",
  dragDestination: "Choose drag destination",
  dragConfirmation: "Confirm drag",
  executing: "Performing drag",
  row: "Choose a row",
  cell: "Choose a cell",
  rowEscape: "Select to return to rows",
  x: "Choose the horizontal position",
  y: "Choose the vertical position",
};

export function ScanningSection({ controller }: { controller: ScanningController }) {
  const { state, config, pending, error, retry, unsaved } = controller;
  return <div className="scanner-settings">
    <div className="scanner-status">
      <p role="status">{state?.enabled ? `${state.paused ? "Paused. " : ""}${phases[state.phase]}${state.phase === "mouseMoving" || state.phase === "mouseScrolling" ? `. ${mouseRepeatStopInstruction(state.config)}` : ""}.` : (state?.message ?? "Loading point scan...")}</p>
      <p role="status" className="setting-note">{pending ? "Saving scanning settings..." : unsaved ? "Scanning settings have unsaved changes." : "Saved automatically."}</p>
      {error && <p role="alert">{error}</p>}
      {error && unsaved && <Button type="button" className="secondary" disabled={!!pending || !validSwitches(config)} onClick={retry}>Retry save</Button>}
      {!validSwitches(config) && <p role="alert">Each switch action needs a different key. Escape is reserved for cancel.</p>}
    </div>
    <ScannerPreferences controller={controller} />
    <details className="scanner-help"><summary>How scanning works</summary>
      <p>Point scanning chooses a screen location with a moving line or grid, then opens actions for clicks, scrolling and dragging. Mouse scanning moves a visible pointer ring and offers directions, clicks, dragging, scrolling and Keyboard in a persistent panel.</p>
      <p>Select opens Home, unless you chose Last mode used. Home offers Point, Mouse and Keyboard, plus Apps and windows, Editing, Browser and Media commands that act on the app in front, so you can bring the right app forward with switches instead of focusing it first. After a click or other action, Select carries on in the same mode; Close menu, Stop scanning or Escape means the next Select opens Home. Home is also a tile in the Point action menu and Mouse Actions.</p>
      <p>Choose Mouse from Home or the Point action menu, or assign Open mouse to a switch. Mouse has an Actions tile for editing, windows, browser, media and other commands, so you can stay in Mouse. Modified clicks use the current pointer position. Choose Switch to Point in the Mouse panel or assign Open point to a switch. Stop scanning or Escape ends the session without changing the mode Select continues in next time.</p>
      <p>In Mouse, select a direction to move with the saved pointer speed and Remote repeat settings. {mouseRepeatStopInstruction(config)} repeating and scan the panel again. If repeat is off, each selection moves one step.</p>
      <p>Assign switch actions in the Switches page. Actions run on release; holding a switch pauses the scan highlight. During repeating Mouse movement or scrolling, {config.mouseRepeatStopEdge === "press" ? "the first switch press stops the action" : "the stop switch keeps the action going until release"}; the stop gesture ignores assigned actions. Manual scanning needs Select, Next and Previous. After clicking or reaching the pass limit, use Select to start again.</p>
      <p>The scanning keyboard offers QWERTY, Simple QWERTY, and Common letters first. Simple QWERTY keeps three letter rows with punctuation on Numbers. Choose a saved layout in Keyboard settings or cycle layouts from the keyboard footer while typing.</p>
      <p>Assigned keys stay reserved while Switchify runs. Escape resets the scan. Mobile connections pause local scanning until they end.</p>
      <p>On Windows, assigned keys remain switches with Shift, Ctrl, Alt or Windows held, even in the background. Other keys and Switchify-generated shortcuts still work normally.</p>
    </details>
    <Demonstration kind="grid" /><Demonstration kind="line" /><Demonstration kind="hold" />
  </div>;
}
