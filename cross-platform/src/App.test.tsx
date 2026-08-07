import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { api, browserPreviewState } from "./api";

describe("Switchify PC Preview shell", () => {
  it("uses the Switchify application icon in the sidebar", async () => {
    const { container } = render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    const brand = container.querySelector(".brand");
    expect(brand).toHaveTextContent("SwitchifyPC Preview");
    expect(brand?.querySelector("img.brand-mark")).toHaveAttribute("src", expect.stringContaining("icon.png"));
    expect(brand?.querySelector("img.brand-mark")).toHaveAttribute("alt", "");
  });

  it("renders the connection and permission state", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Switchify PC" })).toBeInTheDocument();
    expect(screen.getByText("Input access")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Accessibility Settings" })).toBeInTheDocument();
  });

  it("opens settings with accessible native controls", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "100% pointer speed" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("checkbox", { name: "Repeat mouse movement" })).toBeChecked();
    expect(screen.getByRole("group", { name: "Movement acceleration" })).not.toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Medium" })[0]).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    expect(screen.getByRole("button", { name: "50% pointer speed" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("2.5")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Exact pointer speed" }), { target: { value: "125" } });
    expect(screen.getByRole("combobox", { name: "Exact pointer speed" })).toHaveValue("125");
    expect(screen.getByText("5.5")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: "Repeat mouse movement" }));
    expect(screen.getByRole("group", { name: "Movement acceleration" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Show cursor overlay" })).toBeChecked();
    expect(screen.getByRole("button", { name: "While controlling" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/On input hides shortly/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "On input" }));
    expect(screen.getByRole("button", { name: "On input" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getAllByRole("button", { name: "Medium" }).at(-1)).toHaveAttribute("aria-pressed", "true");
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

  it("guides macOS users through required and stale accessibility entries", async () => {
    const originalPlatform = browserPreviewState.capabilities.platform;
    browserPreviewState.capabilities.platform = "macos";
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Support" }));
    expect(screen.getByText(/Enable “Switchify PC Preview” in Accessibility/)).toBeInTheDocument();
    expect(screen.getByText(/select the stale row, click Remove/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Accessibility Settings" })).toBeInTheDocument();
    browserPreviewState.capabilities.platform = originalPlatform;
  });

  it("updates accessibility to Ready from the runtime event", async () => {
    let stateHandler: ((state: typeof browserPreviewState) => void) | undefined;
    const listener = vi.spyOn(api, "onState").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    stateHandler?.({ ...structuredClone(browserPreviewState), accessibility: "granted" });
    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open Accessibility Settings" })).not.toBeInTheDocument();
    listener.mockRestore();
  });
});
