import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("Switchify PC Preview shell", () => {
  it("renders the connection and permission state", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Switchify PC" })).toBeInTheDocument();
    expect(screen.getByText("Input access")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Review" })).toBeInTheDocument();
  });

  it("opens settings with accessible native controls", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: /Pointer speed/ })).toHaveValue("100");
    expect(screen.getByRole("checkbox", { name: "Show cursor overlay" })).toBeChecked();
    expect(screen.getByRole("button", { name: "On input" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Medium" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("radio", { name: "Red" })).toBeChecked();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show crosshairs" }));
    expect(screen.getByRole("checkbox", { name: "Show crosshairs" })).toBeChecked();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show cursor overlay" }));
    expect(screen.getByRole("checkbox", { name: "Show crosshairs" })).toBeDisabled();
  });

  it("creates a profile and records a desired key", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Switch 1 action" }), { target: { value: "shortcut" } });
    const recorder = screen.getByRole("textbox", { name: "Switch 1 key" });
    fireEvent.keyDown(recorder, { key: "K", ctrlKey: true });
    expect(recorder).toHaveValue("Ctrl+K");
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    expect(await screen.findByText("New profile")).toBeInTheDocument();
  });

  it("exposes setup status and troubleshooting actions", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Support" }));
    expect(screen.getByRole("heading", { name: "Android device" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Troubleshooting" }));
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Application update" })).toBeInTheDocument();
  });
});
