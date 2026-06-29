//! Datei-Share-Zugriff: Config, Master-CSV, Inventar-JSONs, Zuordnungen.
//! Fuehrt alles zu DeviceFull zusammen und aggregiert die Overview.
mod assignments;
mod atomic;
mod common;
mod config;
mod inventory;
mod master_csv;
mod merge;
mod overview;
mod text;

pub use assignments::{write_assignment_for_known_hosts, AssignmentWrite};
pub use config::{load_config, save_config};
pub use merge::{apply_manual_assignment, build_devices};
pub use overview::build_overview;

#[cfg(test)]
mod io_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
