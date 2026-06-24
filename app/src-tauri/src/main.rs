// HardView — Desktop-Einstiegspunkt. Im Release-Build ohne Konsolenfenster.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    hardview_lib::run()
}
