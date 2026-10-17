use super::launch_log::LaunchLog;
use serde_json::json;
use std::fs;

#[test]
fn preflight_retains_the_previous_launch_failure() {
    let root = tempfile::tempdir().unwrap();
    let original = "original launch failure\n";
    fs::write(root.path().join("launch.log"), original).unwrap();
    let mut log = LaunchLog::preflight(root.path()).unwrap();
    log.record("failed", json!({"error":"access denied"})).unwrap();
    assert_eq!(fs::read_to_string(root.path().join("launch.log")).unwrap(), original);
    let event: serde_json::Value = serde_json::from_str(&fs::read_to_string(root.path().join("preflight.log")).unwrap()).unwrap();
    assert_eq!(event["detail"]["error"], "access denied");
}
