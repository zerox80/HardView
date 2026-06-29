mod ad;
mod commands;
mod export;
mod identity;
mod model;
mod store;
mod upgrade;

#[cfg(test)]
mod commands_tests;
#[cfg(test)]
mod golden_tests;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_devices,
            commands::get_device,
            commands::get_overview,
            commands::get_ad_users,
            commands::set_assignment,
            commands::refresh,
            commands::get_settings,
            commands::set_settings,
            commands::me,
            commands::export_devices,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Start von HardView");
}
