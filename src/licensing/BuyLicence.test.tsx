import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { openUrl } from "@tauri-apps/plugin-opener";
import { BuyLicence, licencePurchaseUrl } from "./BuyLicence";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("opens the approved live checkout only after the button is selected", async () => {
  vi.mocked(openUrl).mockResolvedValue();
  render(<BuyLicence />);
  expect(openUrl).not.toHaveBeenCalled();
  expect(screen.queryByText(/Test checkout/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Buy licence" }));
  await waitFor(() => expect(openUrl).toHaveBeenCalledWith(licencePurchaseUrl));
});

it("reports a browser launch failure and allows another attempt", async () => {
  vi.mocked(openUrl).mockRejectedValueOnce(new Error("unavailable")).mockResolvedValueOnce();
  render(<BuyLicence />);
  fireEvent.click(screen.getByRole("button", { name: "Buy licence" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not open checkout");
  fireEvent.click(screen.getByRole("button", { name: "Buy licence" }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  expect(openUrl).toHaveBeenCalledTimes(2);
});
