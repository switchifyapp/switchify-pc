import { render, screen } from "@testing-library/react";
import { Crosshair, X, Zap } from "lucide-react";
import { expect, it } from "vitest";
import { HomeTileContent, homePresentation } from "./homePresentation";

it("keeps a renamed tile's icon tied to its action and its label readable", () => {
  const action = { type: "openPanel", panel: "point" } as const;
  expect(homePresentation(action)).toEqual({ Icon: Crosshair, navigation: false });
  render(<HomeTileContent action={action} label="Mouse" />);
  expect(screen.getByText("Mouse")).toBeInTheDocument();
  expect(homePresentation({ type: "close" })).toEqual({ Icon: X, navigation: true });
  expect(homePresentation({ type: "text", text: "Hello" })).toEqual({ Icon: Zap, navigation: false });
});
