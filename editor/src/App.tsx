import DictionaryExplorer, { DictionaryItem } from "./DictionaryExplorer";
import DictionaryEditor from "./DictionaryEditor";
import React from "react";

function App() {
  const [dictionary, setDictionary] = React.useState<DictionaryItem>();

  return (
    <>
      {dictionary == null && (
        <DictionaryExplorer onSelectDictionary={setDictionary} />
      )}
      {dictionary && (
        <DictionaryEditor
          dictionary={dictionary}
          onBack={() => setDictionary(undefined)}
        />
      )}
    </>
  );
}

export default App;
