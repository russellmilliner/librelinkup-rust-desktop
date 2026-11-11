#[test]
fn test_overlay_and_minimized_window_task_manager_icon_exclusion() {
    let dir = std::env::temp_dir().join(format!("llu_taskbar_test_cfg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("LIBRELINKUP_CONFIG_DIR", &dir);
    std::env::set_var("LIBRELINKUP_DATA_DIR", &dir);

    let report =
        librelinkup_kde::bridge::qobject_bridge::qobject::librelinkup_run_task_manager_icon_tests()
            .to_string();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        report, "OK",
        "KDE Plasma Task Manager icon exclusion test failed:\n{report}"
    );
}
