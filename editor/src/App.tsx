import { FluentProvider, webLightTheme } from "@fluentui/react-components";
import DictionaryExplorer, { DictionaryItem } from "./DictionaryExplorer";
import DictionaryEditor from "./DictionaryEditor";
import React from "react";

function App() {
  const [dictionary, setDictionary] = React.useState<DictionaryItem>();

  return (
    <FluentProvider theme={webLightTheme}>
      {dictionary == null && <DictionaryExplorer onSelectDictionary={setDictionary} />}
      {dictionary && <DictionaryEditor dictionary={dictionary} onBack={() => setDictionary(undefined)} />}
    </FluentProvider>
  );
}

export default App;
