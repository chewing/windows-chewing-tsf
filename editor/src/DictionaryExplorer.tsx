import { Button, createTableColumn, DataGrid, DataGridBody, DataGridHeader, DataGridHeaderCell, DataGridRow, makeStyles, TableColumnDefinition } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import DictionaryDetail, { DictionaryDetailProps } from "./DictionaryDetail";
import { invoke } from "@tauri-apps/api/core";
import { message, open, save } from "@tauri-apps/plugin-dialog";

type DictionaryExplorerProps = {
    onSelectDictionary?: (dictionary: DictionaryItem | undefined) => void;
};

const useStyles = makeStyles({
    root: {
        padding: "10px",
        display: "flex",
    },
    leftPanel: {
        flex: 1,
        minWidth: "50%",
        display: "flex",
        flexDirection: "column",
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
});

type DictionaryItem = {
    category: string;
    name: string;
    path: string;
};

const columns: TableColumnDefinition<DictionaryItem>[] = [
    createTableColumn<DictionaryItem>({
        columnId: "type",
        renderHeaderCell: () => <b>類型</b>,
        renderCell: (item) => item.category,
    }),
    createTableColumn<DictionaryItem>({
        columnId: "name",
        renderHeaderCell: () => <b>名稱</b>,
        renderCell: (item) => item.name,
    }),
    createTableColumn<DictionaryItem>({
        columnId: "path",
        renderHeaderCell: () => <b>路徑</b>,
        renderCell: (item) => item.path,
    }),
];

function DictionaryExplorer(props: DictionaryExplorerProps) {
    const styles = useStyles();
    const [items, setItems] = useState<DictionaryItem[]>([]);
    const [selected, setSelected] = useState<DictionaryItem>();
    const [dictInfo, setDictInfo] = useState<DictionaryDetailProps>();

    const reload = () => {
        invoke("explore").then((value) => {
            const dicts = value as DictionaryItem[];
            setItems(dicts);
            setSelected(undefined);
        }).catch((e) => {
            message(e, { title: "錯誤", kind: "error" })
        });
    };

    useEffect(reload, []);

    useEffect(() => {
        if (selected === undefined) {
            setDictInfo(undefined);
            return;
        }
        invoke("info", { path: selected?.path }).then((value) => {
            const info = value as Omit<DictionaryDetailProps, "category">;
            setDictInfo({
                category: selected!.category,
                ...info
            })
        }).catch((e) => {
            message(e, { title: "錯誤", kind: "error" })
        });
    }, [selected]);

    const import_file = async () => {
        await message("匯入字典檔會覆蓋現有字典資料", {
            title: "警告",
            kind: "warning",
        });
        const path = await open({
            filters: [{
                name: "CSV",
                extensions: ["csv"]
            }]
        });
        if (path) {
            invoke("import_file", { path: path }).catch((e) => {
                message(e, { title: "錯誤", kind: "error" })
            });
        }
    };

    const export_file = async () => {
        const path = await save({
            filters: [{
                name: "CSV",
                extensions: ["csv"]
            }]
        });
        if (path) {
            invoke("export_file", { path: path }).catch((e) => {
                message(e, { title: "錯誤", kind: "error" })
            });
        }
    };

    return (
        <div className={styles.root}>
            <div className={styles.leftPanel}>
                <div className={styles.topPanel}>
                    <Button disabled={selected === undefined} onClick={() => { props?.onSelectDictionary && props?.onSelectDictionary(selected) }}>編輯</Button>
                    <Button onClick={import_file}>匯入</Button>
                    <Button onClick={export_file}>匯出</Button>
                    <Button onClick={reload}>重新整理</Button>
                </div>
                <div className={styles.bottomPanel}>
                    <DataGrid
                        items={items}
                        columns={columns}
                        resizableColumns={true}
                        columnSizingOptions={{
                            "type": { defaultWidth: 48, autoFitColumns: false, minWidth: 48, idealWidth: 48, },
                            "name": { defaultWidth: 96, autoFitColumns: true, minWidth: 96, idealWidth: 96, },
                            "path": { defaultWidth: 96, autoFitColumns: true },
                        }}
                        selectionMode="single"
                        onSelectionChange={(_e, data) => setSelected(items[data.selectedItems.values().next().value as number || 0])}>
                        <DataGridHeader>
                            <DataGridRow>
                                {({ renderHeaderCell }) => (
                                    <DataGridHeaderCell>{renderHeaderCell()}</DataGridHeaderCell>
                                )}
                            </DataGridRow>
                        </DataGridHeader>
                        <DataGridBody<DictionaryItem>>
                            {({ item, rowId }) => (
                                <DataGridRow<DictionaryItem> key={rowId}>
                                    {({ renderCell }) => (
                                        <DataGridHeaderCell>{renderCell(item)}</DataGridHeaderCell>
                                    )}
                                </DataGridRow>
                            )}
                        </DataGridBody>
                    </DataGrid>
                </div>
            </div>
            {dictInfo && <div className={styles.rightPanel}>
                <DictionaryDetail {...dictInfo} />
            </div>}
        </div>
    );
}

export default DictionaryExplorer;
export type { DictionaryItem };