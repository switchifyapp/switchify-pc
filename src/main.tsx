import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { isModifierOverlayRoute, ModifierOverlay } from "./ModifierOverlay";
import "./styles.css";

const modifierOverlay = isModifierOverlayRoute(window.location.search);
if (modifierOverlay) document.documentElement.classList.add("modifier-overlay-document");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{modifierOverlay ? <ModifierOverlay /> : <App />}</React.StrictMode>,
);
