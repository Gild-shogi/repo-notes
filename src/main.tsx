import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { NoteDraftProvider } from "./lib/noteDrafts";

if (import.meta.env.DEV) {
  const { installDevMock } = await import("./lib/devMock");
  installDevMock();
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <NoteDraftProvider><App /></NoteDraftProvider>
  </React.StrictMode>,
);
