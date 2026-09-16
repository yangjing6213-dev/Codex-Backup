#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|argument| argument == "--run-due") {
        match rehome_desktop_lib::backup_workflow::run_due_backup() {
            Ok(snapshot) => {
                if snapshot.complete {
                    println!(
                        "scheduled local backup completed: {}",
                        snapshot.restic_snapshot_id
                    );
                } else {
                    eprintln!(
                        "scheduled local backup is partial: {}",
                        snapshot.restic_snapshot_id
                    );
                    std::process::exit(2);
                }
            }
            Err(error) => {
                eprintln!("scheduled local backup failed: {}", error.message);
                std::process::exit(1);
            }
        }
    } else {
        rehome_desktop_lib::run();
    }
}
