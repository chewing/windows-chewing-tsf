import { createTableColumn, Button, Input, makeStyles, TableColumnDefinition, OnSelectionChangeData, InputOnChangeData } from "@fluentui/react-components";
import { DataGrid, DataGridBody, DataGridHeader, DataGridHeaderCell, DataGridRow, RowRenderer } from "@fluentui-contrib/react-data-grid-react-window";
import { useEffect, useState } from "react";
import PhraseEditor from "./PhraseEditor";
import { DictionaryItem } from "./DictionaryExplorer";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";

const useStyles = makeStyles({
    root: {
        padding: "10px",
        display: "flex",
        "& .bopomofo": {
            fontFamily: "標楷體"
        }
    },
    leftPanel: {
        flex: 1,
        minWidth: "50%",
        display: "flex",
        flexDirection: "column",
        gap: "10px",
    },
    topPanel: {
        display: "flex",
        flexDirection: "row",
        gap: "5px",
    },
    bottomPanel: {
        marginTop: "20px",
        overflowX: "hidden",
    },
    rightPanel: {
        flex: 1,
        minWidth: "50%",
        borderLeft: "1px solid #ccc",
        height: "90vh",
    },
    search: {
        width: "90%",
    }
});

type DictionaryEntry = {
    phrase: string;
    bopomofo: string;
    frequency: number;
}

type DictionaryEntryView = DictionaryEntry & {
    index: number;
}

const columns: TableColumnDefinition<DictionaryEntry>[] = [
    createTableColumn<DictionaryEntry>({
        columnId: "phrase",
        renderHeaderCell: () => <b>字/詞</b>,
        renderCell: (item) => item.phrase,
    }),
    createTableColumn<DictionaryEntry>({
        columnId: "reading",
        renderHeaderCell: () => <b>注音</b>,
        renderCell: (item) => <span className="bopomofo">{item.bopomofo}</span>,
    }),
    createTableColumn<DictionaryEntry>({
        columnId: "frequency",
        renderHeaderCell: () => <b>常用度</b>,
        renderCell: (item) => item.frequency.toString(),
    }),
];

function DictionaryEditor(props: { dictionary: DictionaryItem; onBack: () => void }) {
    const styles = useStyles();
    const [filter, setFilter] = useState<string>("");
    const [items, setItems] = useState<DictionaryEntry[]>([]);
    const [itemsView, setItemsView] = useState<DictionaryEntryView[]>([]);
    const [selected, setSelected] = useState<number>();

    const view = (items: DictionaryEntry[], filter: string) =>
        items.map((v, index) => ({ ...v, index })).filter(v => v.phrase.includes(filter));

    useEffect(() => {
        invoke("load", { path: props.dictionary.path }).then((value) => {
            const items = value as DictionaryEntry[];
            setItems(items);
            setItemsView(view(items, ""));
        }).catch((e) => {
            message(e, { title: "錯誤", kind: "error" })
        });
    }, [])

    const renderRow: RowRenderer<DictionaryEntry> = ({ item, rowId }, style) => (
        item.phrase.includes(filter) && <DataGridRow key={rowId} style={style}>
            {({ renderCell }) => (
                <DataGridHeaderCell>{renderCell(item)}</DataGridHeaderCell>
            )}
        </DataGridRow>
    );

    const selectHandler = (_e: any, data: OnSelectionChangeData) => {
        const idx = data.selectedItems.values().next().value as number;
        setSelected(itemsView[idx].index);
    };

    const onInsert = () => {
        const nextItems = [...items];
        nextItems.push({
            phrase: "",
            bopomofo: "",
            frequency: 0,
        });
        setItems(nextItems);
        setFilter("");
        setItemsView(view(nextItems, ""));
        setSelected(nextItems.length - 1);
    }

    const onDelete = () => {
        const nextItems = items.filter((_v, idx) => idx != selected);
        setItems(nextItems);
        setItemsView(view(nextItems, filter));
        setSelected(undefined);
    }

    const onUpdate = (entry: DictionaryEntry) => {
        if (isNaN(entry.frequency)) {
            entry.frequency = 0;
        }
        invoke("validate", { bopomofo: entry.bopomofo }).catch((e) => {
            message(e, { title: "錯誤", kind: "error" })
        });
        const nextItems = [...items];
        nextItems[selected!] = entry;
        setItems(nextItems);
        setItemsView(view(nextItems, filter));
    }

    const onSave = () => {
        invoke("save", { path: props.dictionary.path, entries: items }).then(props.onBack).catch((e) => {
            message(e, { title: "錯誤", kind: "error" })
        });
    }

    const onSearch = (_e: any, data: InputOnChangeData) => {
        setFilter(data.value);
        setItemsView(view(items, data.value));
        setSelected(undefined);
    }

    return (
        <div className={styles.root}>
            <div className={styles.leftPanel}>
                <div className={styles.topPanel}>
                    <Button onClick={onInsert} disabled={props.dictionary.category != "個人"}>新增</Button>
                    <Button onClick={onDelete} disabled={props.dictionary.category != "個人" || selected === undefined}>刪除</Button>
                    <Button onClick={props.onBack}>放棄修改</Button>
                    <Button onClick={onSave} disabled={props.dictionary.category != "個人"}>存檔</Button>
                </div>
                <Input className={styles.search} placeholder="搜尋..." onChange={onSearch} />
                <div className={styles.bottomPanel}>
                    <DataGrid
                        items={itemsView}
                        columns={columns}
                        resizableColumns={true}
                        columnSizingOptions={{
                            "phrase": { defaultWidth: 60, autoFitColumns: true, minWidth: 60, idealWidth: 60, },
                            "bopomofo": { defaultWidth: 100, autoFitColumns: true, minWidth: 100, idealWidth: 150, },
                            "frequency": { defaultWidth: 80, autoFitColumns: true, minWidth: 80, idealWidth: 80, },
                        }}
                        selectionMode="single"
                        subtleSelection={true}
                        onSelectionChange={selectHandler}>
                        <DataGridHeader>
                            <DataGridRow>
                                {({ renderHeaderCell }) => (
                                    <DataGridHeaderCell>{renderHeaderCell()}</DataGridHeaderCell>
                                )}
                            </DataGridRow>
                        </DataGridHeader>
                        <DataGridBody<DictionaryEntry> itemSize={40} height={400}>
                            {renderRow}
                        </DataGridBody>
                    </DataGrid>
                </div>
            </div>
            <div className={styles.rightPanel}>
                <PhraseEditor key={selected}
                    disabled={selected === undefined}
                    phrase={selected !== undefined ? items[selected].phrase : ""}
                    bopomofo={selected !== undefined ? items[selected].bopomofo : ""}
                    frequency={selected !== undefined ? items[selected].frequency : 0}
                    onChange={onUpdate} />
            </div>
        </div>
    );
}

export default DictionaryEditor;
export type { DictionaryEntry };