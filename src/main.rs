#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::process::exit(minecraft_server_downloader::shell::webview_host::run());
}
