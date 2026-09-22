import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { expect, it } from "vitest";
import { Input, MoreOptions } from "./controls";

it("keeps an unfinished edit when options are closed and reopened", () => {
  function Editor() {
    const [value, setValue] = useState("Saved name");
    return <Input aria-label="Name" value={value} onChange={event => setValue(event.target.value)} />;
  }
  render(<MoreOptions><Editor /></MoreOptions>);
  const toggle = screen.getByRole("button", { name: "More options" });
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(screen.queryByRole("textbox")).toBeNull();
  fireEvent.click(toggle);
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "Unfinished name" } });
  fireEvent.click(toggle);
  expect(input).not.toBeVisible();
  fireEvent.click(toggle);
  expect(screen.getByRole("textbox")).toBe(input);
  expect(input).toHaveValue("Unfinished name");
});

it("reveals an error that arrives in a collapsed section", async () => {
  const view = render(<MoreOptions><div role="alert" /></MoreOptions>);
  view.rerender(<MoreOptions><div role="alert">Could not save</div></MoreOptions>);
  await waitFor(() => expect(screen.getByRole("alert")).toBeVisible());
  expect(screen.getByRole("button")).toHaveAttribute("aria-expanded", "true");
});

it("reveals a field that becomes invalid without error text", async () => {
  const view = render(<MoreOptions><Input aria-label="Name" /></MoreOptions>);
  view.rerender(<MoreOptions><Input aria-label="Name" aria-invalid="true" /></MoreOptions>);
  await waitFor(() => expect(screen.getByRole("textbox", { name: "Name" })).toBeVisible());
});

it("reveals an editor needing attention and links the toggle to its content", () => {
  const view = render(<MoreOptions attention={false}><Input aria-label="Name" /></MoreOptions>);
  view.rerender(<MoreOptions attention><Input aria-label="Name" /></MoreOptions>);
  const toggle = screen.getByRole("button");
  expect(document.getElementById(toggle.getAttribute("aria-controls")!)).toContainElement(screen.getByRole("textbox"));
  expect(toggle).toHaveAttribute("aria-expanded", "true");
});
