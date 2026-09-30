use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// The latest launch only: bounded, structured diagnostics, never a full environment dump.
pub(super) struct LaunchLog {
    pub path: PathBuf,
    file: File,
}
impl LaunchLog {
    pub fn new(directory: &Path) -> Result<Self, String> {
        fs::create_dir_all(directory).map_err(|e| format!("无法创建启动日志目录：{e}"))?;
        let path = directory.join("launch.log");
        let file = File::create(&path).map_err(|e| format!("无法创建启动日志：{e}"))?;
        Ok(Self { path, file })
    }

    pub fn record(&mut self, stage: &str, detail: Value) -> Result<(), String> {
        let event = json!({"timeUnixMs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),"stage":stage,"detail":detail});
        writeln!(self.file, "{event}")
            .and_then(|_| self.file.flush())
            .map_err(|e| format!("无法写入启动日志：{e}"))
    }
}
