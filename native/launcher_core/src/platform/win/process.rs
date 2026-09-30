//! Create only our new child suspended; verify its identity before executing it.
use super::target::PackageIdentity;
use serde::Serialize;
use std::{
    cmp::Ordering,
    ffi::{c_void, OsStr, OsString},
    fs::OpenOptions,
    io,
    mem::size_of,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};

type Handle = *mut c_void;
const NO_PACKAGE: i32 = 15700;
const NO_APPLICATION: i32 = 15703;
const WAIT_TIMEOUT: u32 = 258;

#[repr(C)]
#[derive(Default)]
struct StartupInfo {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_chars: u32,
    y_chars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    input: Handle,
    output: Handle,
    error: Handle,
}
#[repr(C)]
struct StartupInfoEx {
    startup: StartupInfo,
    attributes: *mut c_void,
}
#[repr(C)]
#[derive(Default)]
struct ProcessInformation {
    process: Handle,
    thread: Handle,
    process_id: u32,
    thread_id: u32,
}

// Kept in one small FFI boundary so process/thread ownership remains explicit.
#[link(name = "kernel32")]
extern "system" {
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_attributes: *const c_void,
        thread_attributes: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup: *const StartupInfo,
        information: *mut ProcessInformation,
    ) -> i32;
    fn ResumeThread(thread: Handle) -> u32;
    fn TerminateProcess(process: Handle, code: u32) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    fn GetPackageFullName(process: Handle, length: *mut u32, value: *mut u16) -> i32;
    fn GetApplicationUserModelId(process: Handle, length: *mut u32, value: *mut u16) -> i32;
    fn CompareStringOrdinal(
        a: *const u16,
        a_length: i32,
        b: *const u16,
        b_length: i32,
        ignore_case: i32,
    ) -> i32;
    fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> i32;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *const c_void,
        size: usize,
        previous: *mut c_void,
        returned_size: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
}

fn os_error(stage: &str) -> String {
    format!("{stage}：{}", io::Error::last_os_error())
}

fn wide(value: &OsStr) -> Result<Vec<u16>, String> {
    let mut value: Vec<_> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err("启动参数含有无效的空字符。".into());
    }
    value.push(0);
    Ok(value)
}

/// Do not change which file a canonical path refers to when removing its prefix.
fn launch_path(path: &Path) -> OsString {
    let units: Vec<_> = path.as_os_str().encode_wide().collect();
    let prefix: Vec<_> = r"\\?\".encode_utf16().collect();
    if let Some(tail) = units.strip_prefix(prefix.as_slice()) {
        let unc: Vec<_> = "UNC\\".encode_utf16().collect();
        let candidate = if let Some(tail) = tail.strip_prefix(unc.as_slice()) {
            OsString::from_wide(&[&[92, 92], tail].concat())
        } else {
            OsString::from_wide(tail)
        };
        if let Ok(resolved) = std::fs::canonicalize(Path::new(&candidate)) {
            if super::same_path(path, &resolved) {
                return candidate;
            }
        }
    }
    path.as_os_str().into()
}

fn quoted(value: &OsStr, output: &mut Vec<u16>) -> Result<(), String> {
    output.push(34);
    let mut slashes = 0;
    for unit in value.encode_wide() {
        if unit == 0 {
            return Err("启动参数含有无效的空字符。".into());
        }
        if unit == 92 {
            slashes += 1;
            continue;
        }
        output.extend(std::iter::repeat_n(
            92,
            if unit == 34 { slashes * 2 + 1 } else { slashes },
        ));
        slashes = 0;
        output.push(unit);
    }
    output.extend(std::iter::repeat_n(92, slashes * 2));
    output.push(34);
    Ok(())
}

fn child_environment(
    variables: impl Iterator<Item = (OsString, OsString)>,
    timezone: &str,
) -> Result<Vec<u16>, String> {
    let mut entries: Vec<(Vec<u16>, Vec<u16>)> = variables
        .filter(|(key, _)| {
            !["TZ", "ELECTRON_RUN_AS_NODE"]
                .iter()
                .any(|name| key.eq_ignore_ascii_case(name))
        })
        .map(|(key, value)| Ok((wide(&key)?, wide(&value)?)))
        .collect::<Result<_, String>>()?;
    entries.push((wide(OsStr::new("TZ"))?, wide(OsStr::new(timezone))?));
    entries.sort_by(|(a, _), (b, _)| {
        match unsafe {
            CompareStringOrdinal(
                a.as_ptr(),
                (a.len() - 1) as i32,
                b.as_ptr(),
                (b.len() - 1) as i32,
                1,
            )
        } {
            1 => Ordering::Less,
            3 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
    let mut result = Vec::new();
    for (key, value) in entries {
        result.extend_from_slice(&key[..key.len() - 1]);
        result.push(61);
        result.extend(value);
    }
    result.push(0);
    Ok(result)
}

struct Attributes(Vec<usize>);
impl Attributes {
    fn handle_list(handle: &Handle) -> Result<Self, String> {
        let mut size = 0;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size);
        }
        if size == 0 {
            return Err(os_error("无法初始化子进程句柄列表"));
        }
        let mut storage = vec![0; size.div_ceil(size_of::<usize>())];
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 1, 0, &mut size)
        } == 0
        {
            return Err(os_error("无法初始化子进程句柄列表"));
        }
        let mut result = Self(storage);
        if unsafe {
            UpdateProcThreadAttribute(
                result.0.as_mut_ptr().cast(),
                0,
                0x20002,
                (handle as *const Handle).cast(),
                size_of::<Handle>(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(os_error("无法限制子进程继承句柄"));
        }
        Ok(result)
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast());
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ObservedIdentity {
    pub package_error: i32,
    pub package_full_name: Option<String>,
    pub app_id_error: i32,
    pub app_user_model_id: Option<String>,
}

type IdentityQuery = unsafe extern "system" fn(Handle, *mut u32, *mut u16) -> i32;
fn query(process: Handle, function: IdentityQuery) -> (i32, Option<String>) {
    let mut length = 0;
    let mut error = unsafe { function(process, &mut length, ptr::null_mut()) };
    if error != 122 || length == 0 || length > 32768 {
        return (error, None);
    }
    let mut buffer = vec![0; length as usize];
    error = unsafe { function(process, &mut length, buffer.as_mut_ptr()) };
    if error != 0 {
        return (error, None);
    }
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    (0, Some(String::from_utf16_lossy(&buffer[..end])))
}

pub(super) fn require_identity(
    expected: Option<&PackageIdentity>,
    observed: &ObservedIdentity,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if observed.package_error == NO_PACKAGE || observed.app_id_error == NO_APPLICATION {
        return Err(format!("已阻止客户端启动：新进程缺少程序包身份（Windows {} / AppId {}）。请通过 Windows 应用设置修复客户端安装，并核查其 WindowsApps 权限后重试。仅本次新建的挂起进程会被清理，已有客户端不受影响。", observed.package_error, observed.app_id_error));
    }
    if observed.package_error != 0 || observed.app_id_error != 0 {
        return Err(format!(
            "已阻止客户端启动：无法验证新进程的程序包身份（Windows {} / AppId {}）。",
            observed.package_error, observed.app_id_error
        ));
    }
    if observed.package_full_name.as_deref() != Some(expected.full_name.as_str())
        || observed.app_user_model_id.as_deref() != Some(expected.app_user_model_id.as_str())
    {
        return Err(
            "已阻止客户端启动：新进程的程序包身份与所选客户端不一致，请重新自动检测客户端。".into(),
        );
    }
    Ok(())
}

pub(super) struct Child {
    process: OwnedHandle,
    thread: OwnedHandle,
    pub id: u32,
    suspended: bool,
}

impl Child {
    pub fn suspended(
        executable: &Path,
        arguments: &[OsString],
        timezone: &str,
    ) -> Result<Self, String> {
        let program = launch_path(executable);
        let application = wide(&program)?;
        let mut command = Vec::new();
        quoted(&program, &mut command)?;
        for argument in arguments {
            command.push(32);
            quoted(argument, &mut command)?;
        }
        command.push(0);
        let directory = wide(&launch_path(executable.parent().ok_or("客户端目录无效")?))?;
        let environment = child_environment(std::env::vars_os(), timezone)?;
        let null = OpenOptions::new()
            .read(true)
            .write(true)
            .open(r"\\.\NUL")
            .map_err(|e| format!("无法准备客户端输入输出：{e}"))?;
        let null_handle = null.as_raw_handle();
        if unsafe { SetHandleInformation(null_handle, 1, 1) } == 0 {
            return Err(os_error("无法准备客户端输入输出句柄"));
        }
        let mut attributes = Attributes::handle_list(&null_handle)?;
        let startup = StartupInfoEx {
            startup: StartupInfo {
                cb: size_of::<StartupInfoEx>() as u32,
                flags: 0x100,
                input: null_handle,
                output: null_handle,
                error: null_handle,
                ..Default::default()
            },
            attributes: attributes.0.as_mut_ptr().cast(),
        };
        let mut information = ProcessInformation::default();
        // SUSPENDED | UNICODE_ENVIRONMENT | NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT.
        if unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                0x08080404,
                environment.as_ptr().cast(),
                directory.as_ptr(),
                &startup.startup,
                &mut information,
            )
        } == 0
        {
            return Err(os_error("无法创建客户端进程"));
        }
        Ok(Self {
            process: unsafe { OwnedHandle::from_raw_handle(information.process) },
            thread: unsafe { OwnedHandle::from_raw_handle(information.thread) },
            id: information.process_id,
            suspended: true,
        })
    }

    pub fn identity(&self) -> ObservedIdentity {
        let (package_error, package_full_name) =
            query(self.process.as_raw_handle(), GetPackageFullName);
        let (app_id_error, app_user_model_id) =
            query(self.process.as_raw_handle(), GetApplicationUserModelId);
        ObservedIdentity {
            package_error,
            package_full_name,
            app_id_error,
            app_user_model_id,
        }
    }

    pub fn resume(&mut self) -> Result<(), String> {
        let previous = unsafe { ResumeThread(self.thread.as_raw_handle()) };
        if previous == u32::MAX {
            return Err(os_error("无法恢复客户端主线程"));
        }
        if previous == 0 {
            self.suspended = false;
            return Err("客户端主线程已在运行，无法确认预期的恢复执行过程；未终止该进程。".into());
        }
        if previous != 1 {
            return Err(format!(
                "客户端主线程挂起计数异常（{previous}），已取消启动。"
            ));
        }
        self.suspended = false;
        Ok(())
    }

    pub fn abort(&mut self) -> Result<(), String> {
        if !self.suspended {
            return Err("拒绝清理已经恢复执行的客户端进程。".into());
        }
        if unsafe { TerminateProcess(self.process.as_raw_handle(), 1) } == 0 {
            return Err(os_error("无法清理本次挂起进程"));
        }
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), 5000) } != 0 {
            return Err("本次挂起进程的清理尚未完成。".into());
        }
        self.suspended = false;
        Ok(())
    }

    pub fn exit_within(&self, milliseconds: u32) -> Result<Option<u32>, String> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), milliseconds) } {
            WAIT_TIMEOUT => Ok(None),
            0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
                    return Err(os_error("无法读取客户端退出状态"));
                }
                Ok(Some(code))
            }
            _ => Err(os_error("无法观察客户端进程状态")),
        }
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        // Fail closed on all early-return paths, but never stop a resumed/existing app.
        if self.suspended {
            let _ = self.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[link(name = "shell32")]
    extern "system" {
        fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    #[test]
    fn quoted_arguments_round_trip_through_windows_parser() {
        let expected = [
            r"C:\测试 路径\Codex.exe",
            "",
            "plain",
            "with spaces",
            r#"quote"inside"#,
            "trailing slash\\",
            r#"before\"quote"#,
        ];
        let mut command = Vec::new();
        for (index, argument) in expected.iter().enumerate() {
            if index > 0 {
                command.push(32);
            }
            quoted(OsStr::new(argument), &mut command).unwrap();
        }
        command.push(0);
        let mut count = 0;
        let arguments = unsafe { CommandLineToArgvW(command.as_ptr(), &mut count) };
        assert!(!arguments.is_null());
        let parsed: Vec<_> = unsafe { std::slice::from_raw_parts(arguments, count as usize) }
            .iter()
            .map(|&argument| {
                let mut length = 0;
                while unsafe { *argument.add(length) } != 0 {
                    length += 1;
                }
                String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(argument, length) })
            })
            .collect();
        unsafe {
            LocalFree(arguments.cast());
        }
        assert_eq!(parsed, expected);
    }

    #[test]
    fn child_environment_replaces_timezone_and_removes_electron_flag_case_insensitively() {
        let variables = [
            ("z", "last"),
            ("tZ", "old"),
            ("electron_run_as_node", "1"),
            ("A", "first"),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), value.into()));
        let block = child_environment(variables, "Asia/Shanghai").unwrap();
        assert_eq!(
            String::from_utf16(&block).unwrap(),
            "A=first\0TZ=Asia/Shanghai\0z=last\0\0"
        );
    }

    #[test]
    fn prefix_is_not_removed_when_equivalence_cannot_be_verified() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(temp.path()).unwrap();
        let missing = canonical.join("missing.exe");
        assert_eq!(launch_path(&missing), missing.as_os_str());
    }

    #[test]
    fn a_missing_or_different_package_is_rejected() {
        let expected = PackageIdentity {
            full_name: "package".into(),
            app_user_model_id: "family!App".into(),
        };
        let mut observed = ObservedIdentity {
            package_error: NO_PACKAGE,
            package_full_name: None,
            app_id_error: NO_APPLICATION,
            app_user_model_id: None,
        };
        assert!(require_identity(Some(&expected), &observed)
            .unwrap_err()
            .contains("15700"));
        assert!(require_identity(None, &observed).is_ok());
        observed.package_error = 0;
        observed.app_id_error = 0;
        observed.package_full_name = Some("other".into());
        observed.app_user_model_id = Some("family!App".into());
        assert!(require_identity(Some(&expected), &observed).is_err());
        observed.package_full_name = Some("package".into());
        assert!(require_identity(Some(&expected), &observed).is_ok());
    }

    #[test]
    fn rejected_suspended_test_process_is_cleaned_without_resuming() {
        // Spawn only the test runner itself, never an installed Codex client.
        let mut child = Child::suspended(
            &std::env::current_exe().unwrap(),
            &["--list".into()],
            "Etc/UTC",
        )
        .unwrap();
        assert_eq!(child.exit_within(0).unwrap(), None);
        let expected = PackageIdentity {
            full_name: "not-the-test-runner".into(),
            app_user_model_id: "fixture!App".into(),
        };
        assert!(require_identity(Some(&expected), &child.identity()).is_err());
        child.abort().unwrap();
        assert_eq!(child.exit_within(0).unwrap(), Some(1));
    }

    #[test]
    fn approved_unpacked_test_process_can_resume_and_exit() {
        let mut child = Child::suspended(
            &std::env::current_exe().unwrap(),
            &["--list".into()],
            "Etc/UTC",
        )
        .unwrap();
        require_identity(None, &child.identity()).unwrap();
        child.resume().unwrap();
        assert_eq!(child.exit_within(10000).unwrap(), Some(0));
    }

    #[test]
    #[ignore = "Explicit local diagnostic only: creates an installed Codex process suspended, verifies identity, then terminates it without ever resuming"]
    fn installed_msix_identity_is_verified_while_suspended() {
        let _runtime = super::super::Runtime::new().unwrap();
        let target = super::super::target::discover().expect("Registered Codex package required");
        let expected = target.package.as_ref().expect("MSIX target required");
        let mut child = Child::suspended(&target.executable, &[], "America/Los_Angeles").unwrap();
        let observed = child.identity();
        let verification = require_identity(Some(expected), &observed);
        let cleanup = child.abort();
        println!(
            "suspended PID={} expected={expected:?} observed={observed:?} cleanup={cleanup:?}",
            child.id
        );
        cleanup.unwrap();
        assert_eq!(child.exit_within(0).unwrap(), Some(1));
        verification.unwrap();
    }
}
