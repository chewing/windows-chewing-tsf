import { Button, FluentProvider, webLightTheme, makeStyles, Body1, Caption1 } from '@fluentui/react-components';
import { getAllWindows, getCurrentWindow } from '@tauri-apps/api/window';
import { openUrl } from '@tauri-apps/plugin-opener';
import { exit } from '@tauri-apps/plugin-process';
import version from './version';

const useStyles = makeStyles({
    root: {
        overflow: "hidden",
    },
    row: {
        margin: "16px",
        display: "flex",
        flexDirection: "row",
        "@media (width < 20em)": {
            flexDirection: "column",
        },
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
        width: "vw",
        margin: "10px",
        justifyContent: "end",
        display: "flex",
        flexDirection: "row",
        gap: "3px",
    }
});

function About() {
    const styles = useStyles();

    const hide_or_exit = async () => {
        const wins = await getAllWindows();
        const main = wins.find(w => w.label == "main");
        const main_is_visible = await main?.isVisible();
        if (!main_is_visible) {
            exit(0);
        } else {
            getCurrentWindow().hide();
        }
    }

    return <FluentProvider theme={webLightTheme}>
        <div className={styles.root}>
            <div className={styles.row}>
                <div className={styles.column}>
                    <figure className={styles.logo}>
                        <img src="logo.png" alt="windows-chewing-tsf logo" />
                        <figcaption><Caption1>題字：翁政銓</Caption1></figcaption>
                    </figure>
                </div>
                <div className={styles.column}>
                    <Body1>新酷音 － 智慧型注音輸入法</Body1>
                    <Body1>版本：{version.productVersion}</Body1>
                    <Body1>發行日期：{version.buildDate}</Body1>
                    <Body1>軟體開發者：libchewing 開發團隊</Body1>
                    <Body1>授權方式：GPL-3.0-or-later</Body1>
                    <Body1>專案首頁：<a href="#about" onClick={() => openUrl("https://chewing.im")}>https://chewing.im</a></Body1>
                </div>
            </div>
            <div className={styles.row}>
                <Body1>新酷音原是 Linux 系統下知名的輸入法，由 gugod, jserv, kanru 等前輩，改良原本由龔律全與陳康本開發的酷音輸入法而來，有多種分支，目前已經能夠支援包括 Mac 在內的多種平台。這個 Windows 版本是由 PCMan, czchen 等人，使用 libchewing 核心，移植出 Windows TSF 版本。</Body1>
            </div>
            <div className={styles.action}>
                <Button onClick={hide_or_exit}>確定</Button>
            </div>
        </div>
    </FluentProvider>;
}

export default About;