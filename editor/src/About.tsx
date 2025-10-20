import { Button, FluentProvider, webLightTheme, makeStyles, Title3 } from '@fluentui/react-components';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { openUrl } from '@tauri-apps/plugin-opener';
import version from './version';

const useStyles = makeStyles({
    root: {
        overflow: "hidden",
    },
    row: {
        margin: "16px",
        display: "flex",
        flexDirection: "row",
    },
    column: {
        display: "flex",
        flexDirection: "column",
    },
    logo: {
        margin: 0,
        marginRight: "16px",
    },
    action: {
        position: "absolute",
        bottom: "16px",
        right: "16px",
        display: "flex",
        flexDirection: "row",
        gap: "3px",
    }
});

function About() {
    const styles = useStyles();

    return <FluentProvider theme={webLightTheme}>
        <div className={styles.root}>
            <Title3>新酷音詞庫管理程式</Title3>
            <p>
                版本：{version.productVersion}<br/>
                發行日期：{version.buildDate}<br/>
                版權所有© 2024-2025 新酷音開發團隊及 GitHub 貢獻者。
            </p>
            <p>新酷音詞庫管理程式是一個跨平台的新酷音詞庫管理及編輯工具。它提供了一個簡單的方式來管理使用者詞庫。透過它，使用者可以自訂詞庫以提升輸入效率。</p>
            <p>新酷音詞庫管理程式採用 GNU 通用公眾授權條款第 3.0 版或更新版本授權 (GPL-3.0-or-later)。</p>
            <p>關於新酷音詞庫管理程式的授權詳情，請參閱 <a href="#about" onClick={() => openUrl("https://www.gnu.org/licenses/gpl-3.0.html")}>https://www.gnu.org/licenses/gpl-3.0.html</a> 網站。</p>
            <p>新酷音詞庫管理程式是一個開源專案，開發平台位於 <a href="#about" onClick={() => openUrl("https://github.com/chewing/windows-chewing-tsf")}>https://github.com/chewing/windows-chewing-tsf</a>。歡迎在 issues 提供任何建議。</p>
            <div className={styles.action}>
                <Button onClick={() => getCurrentWindow().hide()}>確定</Button>
            </div>
        </div>
    </FluentProvider>;
}

export default About;