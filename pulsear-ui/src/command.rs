#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Good, {}!", name)
}
