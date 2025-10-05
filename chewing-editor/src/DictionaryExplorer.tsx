import { Button, createTableColumn, DataGrid, DataGridBody, DataGridCell, DataGridHeader, DataGridHeaderCell, DataGridRow, makeStyles, TableColumnDefinition } from "@fluentui/react-components";
import { useState } from "react";
import DictionaryDetail from "./DictionaryDetail";

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

type Item = {
    type: string;
    name: string;
    path: string;
};

const columns: TableColumnDefinition<Item>[] = [
    createTableColumn<Item>({
        columnId: "type",
        renderHeaderCell: () => <b>類型</b>,
        renderCell: (item) => item.type,
    }),
    createTableColumn<Item>({
        columnId: "name",
        renderHeaderCell: () => <b>名稱</b>,
        renderCell: (item) => item.name,
    }),
    createTableColumn<Item>({
        columnId: "path",
        renderHeaderCell: () => <b>路徑</b>,
        renderCell: (item) => item.path,
    }),
];

function DictionaryExplorer() {
    const styles = useStyles();
    const [items, setItems] = useState<Item[]>([
        { type: "系統", name: "chewing.dat", path: "C:\\Windows\\chewing" },
        { type: "使用者", name: "tsi.dat", path: "C:\\Users\\User\\AppData\\Local\\chewing\\custom_dict" },
    ]);

    return (
        <div className={styles.root}>
            <div className={styles.leftPanel}>
                <div className={styles.topPanel}>
                    <Button>編輯</Button>
                    <Button>匯入</Button>
                    <Button>匯出</Button>
                    <Button>重新整理</Button>
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
                        selectionMode="single">
                        <DataGridHeader>
                            <DataGridRow>
                                {({ renderHeaderCell }) => (
                                    <DataGridHeaderCell>{renderHeaderCell()}</DataGridHeaderCell>
                                )}
                            </DataGridRow>
                        </DataGridHeader>
                        <DataGridBody<Item>>
                            {({ item, rowId }) => (
                                <DataGridRow<Item> key={rowId}>
                                    {({ renderCell }) => (
                                        <DataGridHeaderCell>{renderCell(item)}</DataGridHeaderCell>
                                    )}
                                </DataGridRow>
                            )}
                        </DataGridBody>
                    </DataGrid>
                </div>
            </div>
            <div className={styles.rightPanel}>
                <DictionaryDetail type="測試" name="tsi.dat" version="25.8.10" copyright="libchewing core team" license="GPL" software="chewing-cli" />
            </div>
        </div>
    );
}

export default DictionaryExplorer;