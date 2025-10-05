import { FluentProvider, webLightTheme } from "@fluentui/react-components";
import DictionaryExplorer from "./DictionaryExplorer";

function App() {

  return (
    <FluentProvider theme={webLightTheme}>
      <DictionaryExplorer />
    </FluentProvider>
  );
}

export default App;
