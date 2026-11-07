import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLayoutEffect, useRef, useState } from "react";
import { expect, it, vi } from "vitest";
import { ConfirmDialog, Dialog } from "./Dialog";

it("contains Tab and Shift+Tab, skips unavailable controls, and restores the opener", () => {
  function Example() {
    const [open, setOpen] = useState(false);
    return <><button onClick={() => setOpen(true)}>Open</button>{open && <Dialog aria-label="Example" onEscape={() => setOpen(false)}>
      <button disabled>Disabled</button><button hidden>Hidden</button><div style={{ display: "none" }}><button>Invisible</button></div>
      <input aria-label="First" /><textarea aria-label="Middle" /><a href="#last">Last</a>
    </Dialog>}</>;
  }
  render(<Example />);
  const opener = screen.getByRole("button", { name: "Open" });
  opener.focus(); fireEvent.click(opener);
  const dialog = screen.getByRole("dialog");
  expect(dialog).toHaveFocus();
  expect(opener.closest("[inert]")).not.toBeNull();
  fireEvent.keyDown(dialog, { key: "Tab" });
  expect(screen.getByRole("textbox", { name: "First" })).toHaveFocus();
  fireEvent.keyDown(document.activeElement!, { key: "Tab", shiftKey: true });
  expect(screen.getByRole("link", { name: "Last" })).toHaveFocus();
  fireEvent.keyDown(document.activeElement!, { key: "Tab" });
  expect(screen.getByRole("textbox", { name: "First" })).toHaveFocus();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(opener).toHaveFocus(); expect(opener.closest("[inert]")).toBeNull();
});

it("gives only the nested confirmation focus and restores its parent", () => {
  const parentEscape = vi.fn();
  function Example() {
    const [confirm, setConfirm] = useState(false);
    return <Dialog aria-label="Editor" onEscape={parentEscape}><button onClick={() => setConfirm(true)}>Delete</button>
      {confirm && <ConfirmDialog title="Delete item?" action="Delete item" onCancel={() => setConfirm(false)} onConfirm={vi.fn()} />}
    </Dialog>;
  }
  render(<Example />);
  const parent = screen.getByRole("dialog");
  const opener = within(parent).getByRole("button"); opener.focus(); fireEvent.click(opener);
  const confirmation = screen.getByRole("alertdialog");
  expect(within(confirmation).getByRole("button", { name: "Keep editing" })).toHaveFocus();
  expect(parent.closest("[inert]")).not.toBeNull();
  fireEvent.keyDown(document.activeElement!, { key: "Escape" });
  expect(parentEscape).not.toHaveBeenCalled(); expect(opener).toHaveFocus();
  expect(parent.closest("[inert]")).toBeNull();
});

it("keeps a child modal on top when parent and child mount together", () => {
  render(<Dialog aria-label="Parent"><Dialog aria-label="Child"><button>Child action</button></Dialog></Dialog>);
  const child = screen.getByRole("dialog", { name: "Child" });
  expect(child).toHaveFocus();
  const parent = screen.getByRole("dialog", { name: "Parent" });
  expect(parent.closest("[inert]")).not.toBeNull();
  expect(Number(child.parentElement!.style.zIndex)).toBeGreaterThan(Number(parent.parentElement!.style.zIndex));
});

it("preserves pre-existing inert backgrounds after unmount", () => {
  const background = document.createElement("aside"); background.setAttribute("inert", ""); document.body.append(background);
  const view = render(<Dialog aria-label="Modal" />); view.unmount();
  expect(background).toHaveAttribute("inert"); background.remove();
});

it("lets a later independent dialog take ownership over an existing nested dialog", () => {
  const view = render(<><Dialog aria-label="Setup"><Dialog aria-label="Practice" /></Dialog></>);
  view.rerender(<><Dialog aria-label="Setup"><Dialog aria-label="Practice" /></Dialog><Dialog aria-label="Pairing" /></>);
  expect(screen.getByRole("dialog", { name: "Pairing" })).toHaveFocus();
  expect(screen.getByRole("dialog", { name: "Practice" }).closest("[inert]")).not.toBeNull();
});

it("suspends open child dialogs with their parent and resumes ownership", () => {
  const view = render(<><Dialog aria-label="Setup"><Dialog aria-label="Practice" /></Dialog></>);
  view.rerender(<><Dialog aria-label="Setup" suspended><Dialog aria-label="Practice" /></Dialog><Dialog aria-label="Pairing" /></>);
  expect(screen.queryByRole("dialog", { name: "Practice" })).toBeNull();
  expect(screen.getByRole("dialog", { name: "Pairing" })).toHaveFocus();
  view.rerender(<><Dialog aria-label="Setup"><Dialog aria-label="Practice" /></Dialog></>);
  const practice = screen.getByRole("dialog", { name: "Practice" });
  expect(practice).toHaveFocus();
  expect(practice.parentElement).not.toHaveAttribute("inert");
});

it("keeps keyboard learning on the dialog and swallows all keyboard edges", () => {
  const escape = vi.fn();
  const view = render(<Dialog aria-label="Learn" captureKeyboard onEscape={escape}><button tabIndex={-1}>Cancel capture</button></Dialog>);
  const dialog = screen.getByRole("dialog");
  expect(fireEvent.keyDown(dialog, { key: "Tab" })).toBe(false);
  expect(fireEvent.keyUp(dialog, { key: "Enter" })).toBe(false);
  fireEvent.keyDown(dialog, { key: "Escape" }); expect(escape).not.toHaveBeenCalled(); expect(dialog).toHaveFocus();
  view.unmount();
  expect(fireEvent.keyDown(document.body, { key: "Tab" })).toBe(true);
});

it("handles Escape even when a child key recorder prevents its default action", () => {
  const escape = vi.fn();
  render(<Dialog aria-label="Profile" onEscape={escape}><input aria-label="Record key" onKeyDown={(event) => event.preventDefault()} /></Dialog>);
  const recorder = screen.getByRole("textbox"); recorder.focus();
  fireEvent.keyDown(recorder, { key: "Escape" });
  expect(escape).toHaveBeenCalledOnce();
});

it("keeps focus inside a busy confirmation with no enabled controls", () => {
  const cancel = vi.fn();
  function Example() { const ref = useRef<HTMLElement>(null); return <Dialog dialogRef={ref} aria-label="Waiting"><button disabled>Wait</button></Dialog>; }
  render(<Example />); const dialog = screen.getByRole("dialog");
  fireEvent.keyDown(dialog, { key: "Tab" }); expect(dialog).toHaveFocus();
  const view = render(<ConfirmDialog title="Saving" action="Save" busy onCancel={cancel} onConfirm={vi.fn()} />);
  fireEvent.keyDown(screen.getByRole("alertdialog"), { key: "Escape" }); expect(cancel).not.toHaveBeenCalled(); view.unmount();
});

it("restores a capture opener after its parent dialog resumes", async () => {
  function Example() {
    const [capture, setCapture] = useState(false);
    return <Dialog aria-label="Setup" active={!capture}><fieldset disabled={capture}><button onClick={() => setCapture(true)}>Learn key</button></fieldset>
      {capture && <Dialog aria-label="Capture"><button onClick={() => setCapture(false)}>Cancel capture</button></Dialog>}
    </Dialog>;
  }
  render(<Example />);
  const learn = screen.getByRole("button", { name: "Learn key" });
  learn.focus(); fireEvent.click(learn);
  fireEvent.click(screen.getByRole("button", { name: "Cancel capture" }));
  await waitFor(() => expect(learn).toHaveFocus());
});

it("preserves a deliberate focus handoff after capture closes", async () => {
  function Example() {
    const [capture, setCapture] = useState(false);
    const name = useRef<HTMLInputElement>(null);
    useLayoutEffect(() => { if (!capture) name.current?.focus(); }, [capture]);
    return <><button disabled={capture} onClick={() => setCapture(true)}>Learn</button><input ref={name} aria-label="New switch name" />
      {capture && <Dialog aria-label="Capture"><button onClick={() => setCapture(false)}>Done</button></Dialog>}
    </>;
  }
  render(<Example />);
  const learn = screen.getByRole("button", { name: "Learn" }); learn.focus(); fireEvent.click(learn);
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  await waitFor(() => expect(screen.getByRole("textbox")).toHaveFocus());
});
