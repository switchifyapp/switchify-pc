import { Button } from "../ui/controls";
import { Demonstration } from "../help/Demonstration";
import { type ScanningController, validSwitches } from "../scanning/useScanning";
import { ScannerPreferences } from "./ScannerPreferences";

const phases = {
  keyboard: "Choose a keyboard row, then a key",
  keyboardSuspended: "Select to resume the keyboard",
  keyboardOpening: "Opening the keyboard",
  mouse: "Choose a mouse control",
  mouseSuspended: "Select to resume the mouse",
  mouseMoving: "Moving pointer. Press any switch to stop",
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
      <p role="status">{state?.enabled ? `${state.paused ? "Paused. " : ""}${phases[state.phase]}.` : (state?.message ?? "Loading point scan...")}</p>
      <p role="status" className="setting-note">{pending ? "Saving scanning settings..." : unsaved ? "Scanning settings have unsaved changes." : "Saved automatically."}</p>
      {error && <p role="alert">{error}</p>}
      {error && unsaved && <Button type="button" className="secondary" disabled={!!pending || !validSwitches(config)} onClick={retry}>Retry save</Button>}
      {!validSwitches(config) && <p role="alert">Each switch action needs a different key. Escape is reserved for cancel.</p>}
    </div>
    <ScannerPreferences controller={controller} />
    <details className="scanner-help"><summary>How scanning works</summary>
      <p>Focus the application you want to use, then press Select to scan the display under the pointer. Choose a point, then use the action menu for clicks, scrolling and dragging.</p>
      <p>Choose Mouse from the action menu, or assign Open mouse to a switch. The pointer ring stays visible while Mouse is open. Select a direction to move with the saved Remote repeat settings; press any switch to stop repeating and scan the mouse panel again. If repeat is off, each selection moves one step. The panel also offers clicks, dragging, scrolling, speed, monitor navigation and Keyboard.</p>
      <p>Assign switch actions in the Switches page. Actions run on release; holding a switch freezes movement. Manual scanning needs Select, Next and Previous. After clicking or reaching the pass limit, use Select to start again.</p>
      <p>Assigned keys stay reserved while Switchify runs. Escape resets the scan. Mobile connections pause local scanning until they end.</p>
      <p>On Windows, assigned keys remain switches with Shift, Ctrl, Alt or Windows held, even in the background. Other keys and Switchify-generated shortcuts still work normally.</p>
    </details>
    <Demonstration kind="grid" /><Demonstration kind="line" /><Demonstration kind="hold" />
  </div>;
}
