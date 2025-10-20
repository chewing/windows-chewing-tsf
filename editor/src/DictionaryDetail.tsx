import { createTableColumn, DataGrid, DataGridBody, DataGridHeader, DataGridHeaderCell, DataGridRow, makeStyles, TableColumnDefinition } from "@fluentui/react-components";

const useStyles = makeStyles({
    root: {
        padding: "10px",
    },
});

type DictionaryDetailProps = {
    category: string;
    name: string;
    version: string;
    copyright: string;
    license: string;
    software: string;
};

type Item = {
    key: string;
    value: string;
}

const columns: TableColumnDefinition<Item>[] = [
    createTableColumn<Item>({
        columnId: "key",
        renderHeaderCell: () => <b>屬性</b>,
        renderCell: (item) => item.key,
    }),
    createTableColumn<Item>({
        columnId: "value",
        renderHeaderCell: () => <b>內容</b>,
        renderCell: (item) => item.value,
    }),
];

function DictionaryDetail(props: DictionaryDetailProps) {
    const styles = useStyles();

    return (
        <div className={styles.root}>
        <DataGrid
            items={[
                { key: "類型", value: props.category },
                { key: "名稱", value: props.name },
                { key: "版本", value: props.version },
                { key: "版權", value: props.copyright },
                { key: "授權", value: props.license },
                { key: "軟體", value: props.software },
            ]}
            columns={columns}
            resizableColumns={true}
        >
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
    );
}

export default DictionaryDetail;
export type { DictionaryDetailProps };