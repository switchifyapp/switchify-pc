import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { IconButton, Input } from "./controls";
import { FeedbackMessage, FormField } from "./patterns";

it("associates a field with its help and validation message", () => {
  render(<FormField label="Name" hint="Use a short name." error="A name is required.">{(props) => <Input {...props} />}</FormField>);
  const input = screen.getByRole("textbox", { name: "Name" });
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(input).toHaveAccessibleDescription("Use a short name. A name is required.");
  expect(screen.getByRole("alert")).toHaveTextContent("A name is required.");
});

it("uses an alert for errors and a polite status for progress", () => {
  render(<><FeedbackMessage>Could not save.</FeedbackMessage><FeedbackMessage tone="status">Saved.</FeedbackMessage></>);
  expect(screen.getByRole("alert")).toHaveTextContent("Could not save.");
  expect(screen.getByRole("status")).toHaveTextContent("Saved.");
});

it("names an icon action independently of its decorative artwork", () => {
  render(<IconButton label="Remove row"><svg><title>Trash</title></svg></IconButton>);
  expect(screen.getByRole("button", { name: "Remove row" })).toHaveAttribute("title", "Remove row");
  expect(screen.getByRole("button").firstElementChild).toHaveAttribute("aria-hidden", "true");
});
