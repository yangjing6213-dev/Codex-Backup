pub mod backup_workflow;
pub mod commands;
pub mod core;
pub mod workflow;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(workflow::WorkflowState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            workflow::discover_codex,
            workflow::pick_directory,
            workflow::discover_local_candidates,
            workflow::cancel_local_discovery,
            workflow::create_package,
            workflow::inspect_package,
            workflow::build_restore_plan,
            workflow::apply_restore,
            workflow::list_transactions,
            workflow::rollback_transaction,
            workflow::open_path,
            workflow::open_restored_thread,
            backup_workflow::get_app_config,
            backup_workflow::save_app_config,
            backup_workflow::run_local_backup,
            backup_workflow::list_local_backups,
            backup_workflow::restore_local_backup,
            backup_workflow::run_due_backup,
            backup_workflow::get_scheduler_status,
            backup_workflow::set_scheduler,
            backup_workflow::start_cloud_configuration,
            backup_workflow::continue_cloud_configuration,
            backup_workflow::test_cloud_connection,
            backup_workflow::upload_local_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ENHE Codex Backup");
}
