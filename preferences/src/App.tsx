import { Button, FluentProvider, webLightTheme, makeStyles, TabList, Tab, Checkbox, Field, Dropdown, Option, TabValue, SelectTabEvent, SelectTabData, SpinButton, Combobox, Input, Textarea, Text, Radio, CheckboxOnChangeData, OptionOnSelectData, SpinButtonChangeEvent, SpinButtonOnChangeData, InputOnChangeData, RadioGroupOnChangeData, Slider, Tooltip } from '@fluentui/react-components';
import { invoke } from '@tauri-apps/api/core';
import React, { useEffect } from 'react';
import { ChewingTsfConfig, Config } from './config';
import { exit } from '@tauri-apps/plugin-process';
import { listen } from '@tauri-apps/api/event';
import { message, open, save } from '@tauri-apps/plugin-dialog';

const useStyles = makeStyles({
  root: {
    position: "relative",
    margin: "5px 0px 0px 5px",
  },
  content: {
    margin: "16px",
    display: "flex",
    flexDirection: "row",
  },
  column: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    "& .fui-Field": {
      marginBottom: "12px",
    },
  },
  action: {
    position: "absolute",
    bottom: "-4em",
    right: "16px",
    paddingBottom: "1em",
    display: "flex",
    flexDirection: "row",
    gap: "3px",
  },
  textarea: {
    height: "60vh",
  },
  texarea_inner: {
    maxHeight: "unset",
  },
});

function sel_key_type_to_value(sel_key_type: number): string {
  switch (sel_key_type) {
    case 0:
      return '1234567890';
    case 1:
      return 'asdfghjkl;';
    case 2:
      return 'asdfzxcv89';
    case 3:
      return 'asdfjkl789';
    case 4:
      return 'aoeuhtn789';
    case 5:
      return '1234qweras';
    default:
      return '1234567890';
  }
}

function conv_engine_to_value(conv_engine: number): string {
  switch (conv_engine) {
    case 0:
      return '簡單注音';
    case 1:
      return '智慧選詞';
    case 2:
      return '模糊智慧選詞';
    default:
      return '智慧選詞';
  }
}

function simulate_english_layout_to_value(layout: number): string {
  switch (layout) {
    case 0:
      return '無';
    case 1:
      return 'Dvorak';
    case 2:
      return 'Carplx (QGMLWY)';
    case 3:
      return 'Colemak';
    case 4:
      return 'Colemak-DH ANSI';
    case 5:
      return 'Colemak-DH Orth';
    case 6:
      return 'Workman';
    default:
      return '無';
  }
}

function update_channel_to_value(channel: string): string {
  switch (channel) {
    case 'disable':
      return '停用';
    case 'stable':
      return '穩定版';
    case 'development':
      return '預覽版';
    default:
      return '穩定版';
  }
}

function App() {
  const styles = useStyles();

  const [systemFonts, setSystemFonts] = React.useState<string[]>([]);
  const [selectedTab, setSelectedTab] = React.useState<TabValue>("1");
  const [config, setConfig] = React.useState<ChewingTsfConfig>();
  const [symbols_dat, setSymbolsDat] = React.useState<string>("");
  const [swkb_dat, setSwkbDat] = React.useState<string>("");
  const [showAdvanced, setShowAdvanced] = React.useState<boolean>(false);

  useEffect(() => {
    const unlisten_import = listen('import', async () => {
      const file = await open({
        multiple: false,
        filters: [
          { name: 'TOML', extensions: ['toml'] },
        ],
      });
      if (!file) {
        return;
      }
      invoke('import_config', { path: file }).then(value => {
        const cfg = value as Config;
        setConfig(cfg.chewing_tsf);
        setSwkbDat(cfg.swkb_dat);
        setSymbolsDat(cfg.symbols_dat);
      }).catch(async (e) => {
        await message('無法匯入設定檔，請確認檔案格式正確。\n\n' + e, { title: '錯誤', kind: 'error' });
      });
    });
    const unlisten_export = listen('export', async () => {
      const file = await save({
        defaultPath: '新酷音設定.toml',
        filters: [
          { name: 'TOML', extensions: ['toml'] },
        ],
      });
      if (!file) {
        return;
      }
      invoke('export_config', { path: file, config: { chewing_tsf: config, symbols_dat, swkb_dat } }).catch(async (e) => {
        await message('無法寫入檔案。\n\n' + e, { title: '錯誤', kind: 'error' });
      });
    });
    return () => {
      unlisten_import.then((f) => f());
      unlisten_export.then((f) => f());
    };
  }, [config, symbols_dat, swkb_dat]);

  useEffect(() => {
    invoke('load_config').then((value) => {
      const config = value as Config;
      setConfig(config.chewing_tsf);
      setSwkbDat(config.swkb_dat);
      setSymbolsDat(config.symbols_dat);
    });
  }, []);

  useEffect(() => {
    invoke('get_system_fonts').then((value) => {
      const fonts = value as string[];
      setSystemFonts(fonts);
    });
  }, []);

  const onTabSelect = (_event: SelectTabEvent, data: SelectTabData) => {
    setSelectedTab(data.value);
  }

  const setBooleanConfig = (event: React.ChangeEvent<HTMLInputElement>, data: CheckboxOnChangeData) => {
    setConfig({
      ...config,
      [event.target.name]: data.checked
    } as ChewingTsfConfig);
  }

  const setNumberConfig = (name: string, fallback: number) => (_event: SpinButtonChangeEvent, data: SpinButtonOnChangeData) => {
    const displayValue = parseInt(data.displayValue || fallback.toString());
    const value = data.value || (Number.isNaN(displayValue) ? fallback : displayValue);
    console.log(data);
    setConfig({
      ...config,
      [name]: value
    } as ChewingTsfConfig);
  }

  const setStringConfig = (ev: React.ChangeEvent<HTMLInputElement>, data: InputOnChangeData) => {
    setConfig({
      ...config,
      [ev.target.name]: data.value
    } as ChewingTsfConfig);
  }

  const InputBehaviors = () => (
    <div className={styles.content} role="tabpanel" aria-labelledby="InputBehaviors">
      <div className={styles.column}>
        <Checkbox label="使用 Shift 快速切換中英文" name='switch_lang_with_shift' checked={config?.switch_lang_with_shift} onChange={setBooleanConfig} />
        <Checkbox label="使用 CapsLock 快速切換中英文" name='enable_caps_lock' checked={config?.enable_caps_lock} onChange={setBooleanConfig} />
        <Checkbox label="顯示中/英切換通知訊息" name='show_notification' checked={config?.show_notification} onChange={setBooleanConfig} />
        <Checkbox label="使用 Esc 清空編輯區字串" name='esc_clean_all_buf' checked={config?.esc_clean_all_buf} onChange={setBooleanConfig} />
        <Checkbox label="使用 Shift 輸入全形標點符號" name='full_shape_symbols' checked={config?.full_shape_symbols} onChange={setBooleanConfig} />
        <Checkbox label="使用 Shift + Space 快速切換全形英文字母" name='enable_fullwidth_toggle_key' checked={config?.enable_fullwidth_toggle_key} onChange={setBooleanConfig} />
        <Checkbox label="按住 Shift 輸入大寫英文字母" name='upper_case_with_shift' checked={config?.upper_case_with_shift} onChange={setBooleanConfig} />
        <Checkbox label="Ctrl + 數字儲存游標前方的詞" name='add_phrase_forward' checked={config?.add_phrase_forward} onChange={setBooleanConfig} />
        <Checkbox label="啟用向後詞彙選詞模式" name='phrase_choice_rearward' checked={config?.phrase_choice_rearward} onChange={setBooleanConfig} />
        <Checkbox label="按住 Shift 輸入快捷符號" name='easy_symbols_with_shift' checked={config?.easy_symbols_with_shift} onChange={setBooleanConfig} />
        <Checkbox label="按住 Shift + Ctrl 輸入快捷符號" name='easy_symbols_with_shift_ctrl' checked={config?.easy_symbols_with_shift_ctrl} onChange={setBooleanConfig} />
        <Checkbox label="自動學習常用詞與新詞" name='enable_auto_learn' checked={config?.enable_auto_learn} onChange={setBooleanConfig} />
      </div>
      <div className={styles.column}>
        <Checkbox label="使用方向鍵移動游標選字" name='cursor_cand_list' checked={config?.cursor_cand_list} onChange={setBooleanConfig} />
        <Checkbox label="按空白鍵叫出選字視窗" name='show_cand_with_space_key' checked={config?.show_cand_with_space_key} onChange={setBooleanConfig} />
        <Checkbox label="選字完畢自動跳到下一個字" name='advance_after_selection' checked={config?.advance_after_selection} onChange={setBooleanConfig} />
        <Checkbox label="預設以全形模式啟動" name='default_full_space' checked={config?.default_full_space} onChange={setBooleanConfig} />
        <Checkbox label="預設以英文模式啟動" name='default_english' checked={config?.default_english} onChange={setBooleanConfig} />
        <Checkbox label="預設輸出簡體中文（或使用 Ctrl + F12 切換）" name='output_simp_chinese' checked={config?.output_simp_chinese} onChange={setBooleanConfig} />
        <div style={{ marginLeft: "10px", marginTop: "10px" }}>
          <Field label="選字鍵：">
            <Dropdown value={sel_key_type_to_value(config!.sel_key_type)} selectedOptions={[config!.sel_key_type.toString()]}
              onOptionSelect={(_ev, data) => { setConfig({ ...config, sel_key_type: parseInt(data.optionValue || '0') } as ChewingTsfConfig); }}>
              <Option value='0'>1234567890</Option>
              <Option value='1'>asdfghjkl;</Option>
              <Option value='2'>asdfzxcv89</Option>
              <Option value='3'>asdfjkl789</Option>
              <Option value='4'>aoeuhtn789</Option>
              <Option value='5'>1234qweras</Option>
            </Dropdown>
          </Field>
          <Field label="模式：">
            <Dropdown value={conv_engine_to_value(config!.conv_engine)} selectedOptions={[config!.conv_engine.toString()]}
              onOptionSelect={(_ev, data) => { setConfig({ ...config, conv_engine: parseInt(data.optionValue || '1') } as ChewingTsfConfig); }}>
              <Option value='0'>簡單注音</Option>
              <Option value='1'>智慧選詞</Option>
              <Option value='2'>模糊智慧選詞</Option>
            </Dropdown>
          </Field>
          <details open={showAdvanced} onToggle={(ev) => setShowAdvanced(ev.currentTarget.open)}>
            <summary style={{ marginBottom: "10px", cursor: "pointer" }}>進階設定...</summary>
            <Tooltip content="設定按住 Shift 鍵的時間長度，超過此時間視為長壓，取消切換中英模式。" relationship={'label'}>
              <Field label={`Shift 長壓敏感度：${config?.shift_key_sensitivity || 200} ms`}>
                <Slider value={config?.shift_key_sensitivity || 200} min={100} max={1000} step={100}
                  onChange={(_ev, data) => { setConfig({ ...config, shift_key_sensitivity: data.value } as ChewingTsfConfig); }} />
              </Field>
            </Tooltip>
          </details>
        </div>
      </div>
    </div>
  );

  const Appearance = React.memo(() => (
    <div className={styles.content} role="tabpanel" aria-labelledby="Appearance">
      <div className={styles.column}>
        <Field label="每列顯示後選字個數：">
          <SpinButton value={config?.cand_per_row} min={1} max={10} step={1} onChange={setNumberConfig("cand_per_row", 3)} />
        </Field>
        <Field label="每頁顯示後選字個數：">
          <SpinButton value={config?.cand_per_page} min={1} max={10} step={1} onChange={setNumberConfig("cand_per_page", 9)} />
        </Field>
        <Field label="選字及訊息視窗文字大小：">
          <SpinButton value={config?.font_size} step={1} onChange={setNumberConfig("font_size", 16)} />
        </Field>
        <Field label="選字視窗字型：">
          <Combobox value={config?.font_family} selectedOptions={[config?.font_family || '']}
            onOptionSelect={(_ev, data: OptionOnSelectData) => { setConfig({ ...config, font_family: data.optionValue } as ChewingTsfConfig); }} >
            {systemFonts.map((font) => (
              <Option value={font} style={{ fontFamily: font }}>{font}</Option>
            ))}
          </Combobox>
        </Field>
        <details open={showAdvanced} onToggle={(ev) => setShowAdvanced(ev.currentTarget.open)}>
          <summary style={{ marginBottom: "10px", cursor: "pointer" }}>進階設定...</summary>
          <Field label="文字顏色 (RGB)" orientation='horizontal'>
            <Input name="font_fg_color" value={config?.font_fg_color} onChange={setStringConfig} />
          </Field>
          <Field label="選字背景顏色 (RGB)" orientation='horizontal'>
            <Input name="font_bg_color" value={config?.font_bg_color} onChange={setStringConfig} />
          </Field>
          <Field label="焦點文字顏色 (RGB)" orientation='horizontal'>
            <Input name="font_highlight_fg_color" value={config?.font_highlight_fg_color} onChange={setStringConfig} />
          </Field>
          <Field label="焦點背景顏色 (RGB)" orientation='horizontal'>
            <Input name="font_highlight_bg_color" value={config?.font_highlight_bg_color} onChange={setStringConfig} />
          </Field>
          <Field label="數字顏色 (RGB)" orientation='horizontal'>
            <Input name="font_number_fg_color" value={config?.font_number_fg_color} onChange={setStringConfig} />
          </Field>
        </details>
      </div>
    </div>
  ));

  const setLayout = (_ev: any, data: RadioGroupOnChangeData) => {
    setConfig({
      ...config,
      keyboard_layout: parseInt(data.value)
    } as ChewingTsfConfig);
  }

  const Layout = React.memo(() => (
    <div className={styles.content} role="tabpanel" aria-labelledby="Layout">
      <div className={styles.column}>
        <Radio name="layout" value="0" checked={config?.keyboard_layout == 0} label="預設" onChange={setLayout} />
        <Radio name="layout" value="1" checked={config?.keyboard_layout == 1} label="許氏鍵盤" onChange={setLayout} />
        <Radio name="layout" value="2" checked={config?.keyboard_layout == 2} label="IBM 鍵盤" onChange={setLayout} />
        <Radio name="layout" value="3" checked={config?.keyboard_layout == 3} label="精業鍵盤" onChange={setLayout} />
        <Radio name="layout" value="4" checked={config?.keyboard_layout == 4} label="倚天鍵盤" onChange={setLayout} />
        <Radio name="layout" value="5" checked={config?.keyboard_layout == 5} label="倚天 26 鍵" onChange={setLayout} />
      </div>
      <div className={styles.column}>
        <Radio name="layout" value="8" checked={config?.keyboard_layout == 8} label="大千 26 鍵" onChange={setLayout} />
        <Radio name="layout" value="9" checked={config?.keyboard_layout == 9} label="漢語拼音" onChange={setLayout} />
        <Radio name="layout" value="10" checked={config?.keyboard_layout == 10} label="台灣華語羅馬拼音" onChange={setLayout} />
        <Radio name="layout" value="11" checked={config?.keyboard_layout == 11} label="注音二式" onChange={setLayout} />
      </div>
      <div className={styles.column}>
        <details open={showAdvanced} onToggle={(ev) => setShowAdvanced(ev.currentTarget.open)}>
          <summary style={{ marginBottom: "10px", cursor: "pointer" }}>進階設定...</summary>
          <Tooltip content="模擬英文鍵盤布局可能會讓某些網頁快捷鍵失效" relationship={'label'}>
            <Field label={`模擬英文鍵盤布局：`}>
              <Dropdown value={simulate_english_layout_to_value(config?.simulate_english_layout || 0)} selectedOptions={[config?.simulate_english_layout?.toString() || '0']}
                onOptionSelect={(_ev, data) => { setConfig({ ...config, simulate_english_layout: parseInt(data.optionValue || '0') } as ChewingTsfConfig); }}>
                <Option value='0'>無</Option>
                <Option value='1'>Dvorak</Option>
                <Option value='2'>Carplx (QGMLWY)</Option>
                <Option value='3'>Colemak</Option>
                <Option value='4'>Colemak-DH ANSI</Option>
                <Option value='5'>Colemak-DH Orth</Option>
                <Option value='6'>Workman</Option>
              </Dropdown>
            </Field>
          </Tooltip>
        </details>
      </div>
    </div>
  ));

  const Symbols = React.memo(() => (
    <div className={styles.content} role="tabpanel" aria-labelledby="Symbols">
      <div className={styles.column}>
        <Field label="輸入中文時，按下 ` 鍵，會顯示下列的符號表：">
          <Textarea value={symbols_dat} className={styles.textarea} textarea={{ className: styles.texarea_inner }}
            onChange={(_ev, data) => setSymbolsDat(data.value)} />
        </Field>
        <Text>以上是符號表的設定檔，語法相當簡單：<br />每一行的內容都是：「分類名稱」＝「此分類下的所有符號」<br />您也可以一行只放一個符號，則該符號會被放在最上層選單。</Text>
      </div>
    </div>
  ));

  const Shortcut = React.memo(() => (
    <div className={styles.content} role="tabpanel" aria-labelledby="Shortcut">
      <div className={styles.column}>
        <Field label="輸入中文時，按下 Shift 鍵（或 Ctrl + Shift）加英文字母即可快速輸入文字：">
          <Textarea value={swkb_dat} className={styles.textarea} textarea={{ className: styles.texarea_inner }}
            onChange={(_ev, data) => setSwkbDat(data.value)} />
        </Field>
        <Text>以上是符號表的設定檔，語法相當簡單：<br />每一行的內容都是：「大寫字母」＋「空格」＋「對應的符號或文字」。</Text>
      </div>
    </div>
  ));

  const Others = React.memo(() => (
    <div className={styles.content} role="tabpanel" aria-labelledby="Others">
      <div className={styles.column}>
        <Field label="自動檢查更新：">
          <Dropdown value={update_channel_to_value(config?.auto_check_update_channel || 'stable')} selectedOptions={[config?.auto_check_update_channel || 'stable']}
            onOptionSelect={(_ev, data) => { setConfig({ ...config, auto_check_update_channel: data.optionValue || 'stable' } as ChewingTsfConfig); }} >
            <Option value='disable'>停用</Option>
            <Option value='stable'>穩定版</Option>
            <Option value='development'>預覽版</Option>
          </Dropdown>
        </Field>
      </div>
    </div>
  ));

  const save_config = () => invoke("save_config", { config: { chewing_tsf: config, symbols_dat, swkb_dat } });

  return (
    <FluentProvider theme={webLightTheme}>
      <form className={styles.root} name="configs">
        <TabList appearance="subtle" selectedValue={selectedTab} onTabSelect={onTabSelect}>
          <Tab value="1">打字行為</Tab>
          <Tab value="2">界面外觀</Tab>
          <Tab value="3">鍵盤設定</Tab>
          <Tab value="4">特殊符號</Tab>
          <Tab value="5">快捷符號</Tab>
          <Tab value="6">其他設定</Tab>
        </TabList>
        {selectedTab === "1" && <InputBehaviors />}
        {selectedTab === "2" && <Appearance />}
        {selectedTab === "3" && <Layout />}
        {selectedTab === "4" && <Symbols />}
        {selectedTab === "5" && <Shortcut />}
        {selectedTab === "6" && <Others />}
        <div className={styles.action}>
          <Button onClick={() => { save_config().then(() => exit(0)) }}>確定</Button>
          <Button onClick={() => exit(0)}>取消</Button>
          <Button onClick={save_config}>套用</Button>
        </div>
      </form>
    </FluentProvider>
  );
}

export default App;
