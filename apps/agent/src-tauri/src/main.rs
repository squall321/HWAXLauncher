// HWAX Agent — Windows tray-resident module deployment/management agent.
//
// `windows_subsystem = "windows"` (release only) suppresses the console window
// for the GUI/tray app while keeping a console during `tauri dev` for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    hwax_agent_lib::run();
}
