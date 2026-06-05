// Tauri's build script: generates the context (config, capabilities, assets)
// the `tauri::generate_context!()` macro consumes at compile time.
fn main() {
    tauri_build::build();
}
