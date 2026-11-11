#[test]
fn test_desktop_overlay_widget_lifecycle_and_hide_restore_placement() {
    let dir = std::env::temp_dir().join(format!("llu_widget_test_cfg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("LIBRELINKUP_CONFIG_DIR", &dir);
    std::env::set_var("LIBRELINKUP_DATA_DIR", &dir);

    let report =
        librelinkup_kde::bridge::qobject_bridge::qobject::librelinkup_run_desktop_widget_tests()
            .to_string();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        report, "OK",
        "Desktop overlay widget lifecycle or hide->restore placement test failed:\n{report}"
    );
}
