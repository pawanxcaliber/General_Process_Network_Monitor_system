#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub fn main() {
    monitor_lib::run()
}
