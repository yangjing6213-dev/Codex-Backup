#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--run-due") {
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
    } else if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--admin-local-scan")
    {
        let Some(output_path) = arguments.get(index + 1) else {
            eprintln!("administrator scan output path is missing");
            std::process::exit(1);
        };
        if let Err(error) = rehome_desktop_lib::core::local_discovery::run_admin_local_scan(
            std::path::Path::new(output_path),
        ) {
            eprintln!("administrator scan failed: {error}");
            std::process::exit(1);
        }
    } else {
        rehome_desktop_lib::run();
    }
}
