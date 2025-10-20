use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::Manager;

mod explore;
mod editor;
mod file;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let file_menu = SubmenuBuilder::new(app, "檔案")
                .text("quit", "結束")
                .build()?;
            let about_menu = SubmenuBuilder::new(app, "關於")
                .text("about", "關於新酷音詞庫管理程式")
                .build()?;
            let menu = MenuBuilder::new(app)
                .items(&[&file_menu, &about_menu])
                .build()?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_menu(menu)?;
                let app = app.handle().clone();
                window.on_menu_event(move |_window, event| match event.id().0.as_str() {
                    "about" => {
                        if let Some(about_window) = app.get_webview_window("about") {
                            about_window.show().expect("failed to show about window");
                        }
                    },
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            explore::explore,
            explore::info,
            editor::load,
            editor::save,
            editor::validate,
            file::export_file,
            file::import_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
