import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import About from "./About";
import version from "./version";
import { getAllWindows } from "@tauri-apps/api/window";

getAllWindows().then(async (wins) => {
  for (const w of wins) {
    let title = await w.title();
    if (!title.includes(version.productVersion)) {
      w.setTitle(`${title} (${version.productVersion})`);
    }
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {location.hash == '' && <App />}
    {location.hash == '#about' && <About />}
  </React.StrictMode>,
);
