use self::config::export_config;
use self::config::import_config;
use self::config::load_config;
use self::config::save_config;
use self::fonts::get_system_fonts;
use tauri::Emitter;
use tauri::Manager;
use tauri::menu::{MenuBuilder, SubmenuBuilder};

mod config;
mod fonts;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let hide_main = std::env::args().any(|arg| {
                ["/about", "--about", "chewing-preferences://about"].contains(&arg.as_str())
            });
            let file_menu = SubmenuBuilder::new(app, "檔案")
                .text("import", "匯入設定檔...")
                .text("export", "匯出設定檔...")
                .build()?;
            let about_menu = SubmenuBuilder::new(app, "關於")
                .text("about", "關於新酷音輸入法")
                .build()?;
            let menu = MenuBuilder::new(app)
                .items(&[&file_menu, &about_menu])
                .build()?;
            if let Some(window) = app.get_webview_window("main") {
                if hide_main {
                    window.hide().expect("failed to hide main window");
                    if let Some(about_window) = app.get_webview_window("about") {
                        about_window.show().expect("failed to show about window");
                    }
                } else {
                    window.show().expect("failed to show main window");
                }
                window.set_menu(menu)?;
                let app = app.handle().clone();
                window.on_menu_event(move |window, event| match event.id().0.as_str() {
                    "about" => {
                        if let Some(about_window) = app.get_webview_window("about") {
                            about_window.show().expect("failed to show about window");
                        }
                    }
                    "export" => {
                        window
                            .emit("export", ())
                            .expect("failed to emit export event");
                    }
                    "import" => {
                        window
                            .emit("import", ())
                            .expect("failed to emit import event");
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            import_config,
            export_config,
            load_config,
            save_config,
            get_system_fonts
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
