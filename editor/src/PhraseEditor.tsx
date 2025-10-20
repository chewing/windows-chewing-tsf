import { Button, Field, Input, makeStyles } from "@fluentui/react-components";
import Keymap, { Keymaps } from "./keymap";
import { useState } from "react";
import { DictionaryEntry } from "./DictionaryEditor";

type OnPhraseEditorChangeData = {
    phrase: string;
    bopomofo: string;
    frequency: number;
}

type PhraseEditorProps = {
    phrase?: string;
    bopomofo?: string;
    frequency?: number;
    keymap?: Keymap;
    disabled?: boolean,
    onChange?: (data: OnPhraseEditorChangeData) => void;
};

const useStyles = makeStyles({
    root: {
        padding: "10px",
        display: "flex",
        flexDirection: "column",
    },
    keyboard: {
        marginTop: "20px",
        display: "flex",
        flexDirection: "column",
        gap: "5px",
    },
    keyboardRow: {
        display: "flex",
        flexDirection: "row",
        gap: "5px",
        ":nth-child(2)": {
            marginLeft: "10px",
        },
        ":nth-child(3)": {
            marginLeft: "20px",
        },
        ":nth-child(4)": {
            marginLeft: "30px",
        },
        ":nth-child(5)": {
            marginLeft: "40px",
        },
    },
    keycap: {
        minWidth: "35px",
        fontWeight: "normal",
        fontFamily: "標楷體",
    },
    bopomofo: {
        fontFamily: "標楷體",
    }
});

function PhraseEditor(props: PhraseEditorProps) {
    const styles = useStyles();
    const keymap = props.keymap || Keymaps.STD;
    const [entry, setEntry] = useState<DictionaryEntry>({
        phrase: props.phrase || "",
        bopomofo: props.bopomofo || "",
        frequency: props.frequency || 0
    });

    const appendBopomofo = (value: string) => {
        setEntry({
            ...entry,
            bopomofo: entry.bopomofo + value,
        });
    }

    return (
        <div className={styles.root}>
            <Field label="字/詞">
                <Input disabled={props.disabled} value={entry.phrase} onChange={(_ev, data) => setEntry({ ...entry, phrase: data.value })} />
            </Field>
            <Field label="注音">
                <Input className={styles.bopomofo} disabled={props.disabled} value={entry.bopomofo} onChange={(_ev, data) => setEntry({ ...entry, bopomofo: data.value })} />
            </Field>
            <Field label="常用度">
                <Input type="number" disabled={props.disabled} value={entry.frequency.toString()} onChange={(_ev, data) => setEntry({ ...entry, frequency: parseInt(data.value) })} />
            </Field>
            <div className={styles.keyboard}>
                {keymap.layout.map((row, rowId) => (
                    <div key={rowId} className={styles.keyboardRow}>
                        {row.map((key) => (
                            <Button key={key} disabled={props.disabled} className={styles.keycap} onClick={() => appendBopomofo(key)}>{key}</Button>
                        ))}
                    </div>
                ))}
                <div className={styles.keyboardRow}>
                    <Button disabled={props.disabled} onClick={() => setEntry({ ...entry, bopomofo: "" })}>清空注音</Button>
                    <Button disabled={props.disabled} style={{ flex: "1" }} onClick={() => appendBopomofo(" ")}></Button>
                    <Button disabled={props.disabled} appearance="primary" onClick={() => props.onChange && props.onChange(entry)}>確定</Button>
                </div>
            </div>
        </div>
    );
}

export default PhraseEditor;
export type { PhraseEditorProps, OnPhraseEditorChangeData };