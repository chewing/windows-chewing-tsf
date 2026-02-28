import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import About from "./About";
import version from "./version";
import { getAllWindows } from "@tauri-apps/api/window";
import { usePrefersColorScheme } from "./theme";
import {
  FluentProvider,
  webDarkTheme,
  webLightTheme,
} from "@fluentui/react-components";

getAllWindows().then(async (wins) => {
  for (const w of wins) {
    let title = await w.title();
    if (!title.includes(version.productVersion)) {
      w.setTitle(`${title} (${version.productVersion})`);
    }
  }
});

function Root() {
  const isDarkTheme = usePrefersColorScheme();

  return (
    <React.StrictMode>
      <FluentProvider theme={isDarkTheme ? webDarkTheme : webLightTheme}>
        {location.hash == "" && <App />}
        {location.hash == "#about" && <About />}
      </FluentProvider>
    </React.StrictMode>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <Root />,
);
