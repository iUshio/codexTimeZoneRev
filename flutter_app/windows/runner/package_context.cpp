#include "package_context.h"
#include <windows.h>
#include <appmodel.h>
#include <wincrypt.h>
#include <algorithm>
#include <filesystem>
#include <fstream>
#include <sstream>

namespace package_context {
namespace {
constexpr wchar_t kChild[] = L"--codex-context-child=";
constexpr wchar_t kToken[] = L"--codex-context-token=";
constexpr wchar_t kApp[] = L"OpenAI.Codex_2p2nqsd0c76g0!App";
using Path = std::filesystem::path;
struct Handle {
  HANDLE value = nullptr;
  ~Handle() { if (value && value != INVALID_HANDLE_VALUE) CloseHandle(value); }
};
Path Executable() {
  std::wstring buffer(32768, L'\0');
  DWORD n = GetModuleFileNameW(nullptr, buffer.data(), static_cast<DWORD>(buffer.size()));
  if (!n || n >= buffer.size()) return {};
  buffer.resize(n);
  return Path(buffer);
}
std::string Utf8(const std::wstring& text) {
  int n = WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
  std::string result(n, '\0');
  WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), result.data(), n, nullptr, nullptr);
  return result;
}
void Log(const Path& root, const std::wstring& message) {
  std::error_code error;
  std::filesystem::create_directories(root / L"data", error);
  SYSTEMTIME time{}; GetSystemTime(&time);
  std::wostringstream line;
  line << time.wYear << L'-' << time.wMonth << L'-' << time.wDay << L'T'
       << time.wHour << L':' << time.wMinute << L':' << time.wSecond
       << L"Z pid=" << GetCurrentProcessId() << L' ' << message << L"\r\n";
  auto bytes = Utf8(line.str());
  Handle file{CreateFileW((root / L"data" / L"context.log").c_str(), FILE_APPEND_DATA,
      FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr)};
  if (file.value == INVALID_HANDLE_VALUE) return;
  DWORD written = 0;
  WriteFile(file.value, bytes.data(), static_cast<DWORD>(bytes.size()), &written, nullptr);
}
std::wstring PackageName() {
  UINT32 n = 0;
  if (GetCurrentPackageFullName(&n, nullptr) != ERROR_INSUFFICIENT_BUFFER) return {};
  std::wstring value(n, L'\0');
  if (GetCurrentPackageFullName(&n, value.data()) != ERROR_SUCCESS) return {};
  value.resize(n - 1); return value;
}
std::wstring AppId() {
  UINT32 n = 0;
  if (GetCurrentApplicationUserModelId(&n, nullptr) != ERROR_INSUFFICIENT_BUFFER) return {};
  std::wstring value(n, L'\0');
  if (GetCurrentApplicationUserModelId(&n, value.data()) != ERROR_SUCCESS) return {};
  value.resize(n - 1); return value;
}
bool ValidToken(const std::wstring& token) {
  GUID guid{};
  return token.size() == 38 && token.front() == L'{' && token.back() == L'}' &&
      SUCCEEDED(CLSIDFromString(token.c_str(), &guid));
}
Path RunDirectory(const Path& root, const std::wstring& token) {
  return root / L"data" / L"context-bootstrap" / token;
}
void Finish(const Path& root, const std::wstring& token, int code) {
  if (!ValidToken(token)) return;
  auto directory = RunDirectory(root, token);
  if (!std::filesystem::is_directory(directory)) return;
  auto temporary = directory / L"result.tmp";
  { std::ofstream file(temporary, std::ios::binary); file << code; }
  MoveFileExW(temporary.c_str(), (directory / L"result.txt").c_str(), MOVEFILE_REPLACE_EXISTING);
}
int Failure(const Path& root, const std::wstring& message, bool headless) {
  Log(root, L"failed: " + message);
  if (!headless) MessageBoxW(nullptr, (message + L"\n\n诊断日志：" + (root / L"data" / L"context.log").wstring()).c_str(),
      L"Codex 时区启动器", MB_OK | MB_ICONERROR);
  return 1;
}
int Verify(const Path& root) {
  // Read-only native command. It only creates/cleans a suspended client.
  auto library = LoadLibraryExW((root / L"codex_timezone_core.dll").c_str(), nullptr,
      LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
  if (!library) return Failure(root, L"无法加载原生组件。", true);
  using Call = char* (__cdecl*)(const char*);
  using Free = void (__cdecl*)(char*);
  using Version = unsigned int (__cdecl*)();
  auto call = reinterpret_cast<Call>(GetProcAddress(library, "launcher_call"));
  auto free_result = reinterpret_cast<Free>(GetProcAddress(library, "launcher_free"));
  auto version = reinterpret_cast<Version>(GetProcAddress(library, "launcher_abi_version"));
  if (!call || !free_result || !version || version() != 1) {
    FreeLibrary(library); return Failure(root, L"原生组件接口不匹配。", true);
  }
  char* raw = call(R"({"version":1,"command":"check_launch","payload":{}})");
  const std::string result = raw ? raw : "{\"ok\":false}";
  if (raw) free_result(raw);
  FreeLibrary(library);
  // Only exact envelope/result booleans emitted by our JSON backend are accepted.
  bool passed = result.find("\"ok\":true") != std::string::npos &&
      result.find("\"healthy\":true") != std::string::npos;
  { std::ofstream file(root / L"data" / L"context-check.json", std::ios::binary); file << result; }
  Log(root, passed ? L"native_preflight_passed" : L"native_preflight_failed");
  return passed ? 0 : 1;
}
std::wstring Encode(const std::wstring& text) {
  DWORD size = 0;
  auto bytes = reinterpret_cast<const BYTE*>(text.data());
  DWORD length = static_cast<DWORD>(text.size() * sizeof(wchar_t));
  if (!CryptBinaryToStringW(bytes, length, CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF, nullptr, &size)) return {};
  std::wstring encoded(size, L'\0');
  if (!CryptBinaryToStringW(bytes, length, CRYPT_STRING_BASE64 | CRYPT_STRING_NOCRLF, encoded.data(), &size)) return {};
  encoded.resize(size); while (!encoded.empty() && encoded.back() == L'\0') encoded.pop_back();
  return encoded;
}
}

std::wstring QuoteWindows(const std::wstring& value) {
  std::wstring result = L"\"";
  size_t slashes = 0;
  for (wchar_t ch : value) {
    if (ch == L'\\') { ++slashes; continue; }
    result.append(ch == L'"' ? 2 * slashes + 1 : slashes, L'\\');
    slashes = 0; result.push_back(ch);
  }
  result.append(2 * slashes, L'\\'); result.push_back(L'"'); return result;
}
std::wstring QuotePowerShell(const std::wstring& value) {
  std::wstring result = L"'";
  for (wchar_t ch : value) { result.push_back(ch); if (ch == L'\'') result.push_back(ch); }
  result.push_back(L'\''); return result;
}
bool IsCodexIdentity(const std::wstring& package, const std::wstring& app_id) {
  const std::wstring suffix = L"__2p2nqsd0c76g0";
  return package.rfind(L"OpenAI.Codex_", 0) == 0 && package.size() > suffix.size() &&
      package.compare(package.size() - suffix.size(), suffix.size(), suffix) == 0 && app_id == kApp;
}
bool IsInternalArgument(const std::wstring& argument) {
  return argument.rfind(kChild, 0) == 0 || argument.rfind(kToken, 0) == 0;
}

int Prepare(const std::vector<std::wstring>& arguments) {
  const auto executable = Executable();
  const auto root = executable.parent_path();
  const bool verify = std::find(arguments.begin(), arguments.end(), L"--verify-launch") != arguments.end();
  if (!verify && std::find(arguments.begin(), arguments.end(), L"--preview") != arguments.end()) return -1;
  std::wstring expected, token;
  for (const auto& arg : arguments) {
    if (arg.rfind(kChild, 0) == 0) expected = arg.substr(std::size(kChild) - 1);
    if (arg.rfind(kToken, 0) == 0) token = arg.substr(std::size(kToken) - 1);
  }
  bool internal = std::any_of(arguments.begin(), arguments.end(), IsInternalArgument);
  auto package = PackageName();
  bool correct = IsCodexIdentity(package, AppId());
  if (internal) {
    if (!ValidToken(token) || expected.empty() || !correct || package != expected ||
        !std::filesystem::is_directory(RunDirectory(root, token))) {
      Finish(root, token, 1);
      return Failure(root, L"包身份交接校验失败，已停止自动重试。", verify);
    }
    if (std::filesystem::exists(RunDirectory(root, token) / L"cancel")) {
      Finish(root, token, 1); return Failure(root, L"包身份交接已超时，已停止启动。", verify);
    }
  }
  if (correct) {
    Log(root, L"context_verified package=" + package);
    int code = verify ? Verify(root) : -1;
    Finish(root, token, code == -1 ? 0 : code);
    return code;
  }

  GUID guid{}; if (FAILED(CoCreateGuid(&guid))) return Failure(root, L"无法建立启动请求。", verify);
  wchar_t guid_text[40]{}; StringFromGUID2(guid, guid_text, 40); token = guid_text;
  auto run = RunDirectory(root, token);
  std::error_code error; std::filesystem::create_directories(run, error);
  if (error) return Failure(root, L"无法创建启动诊断目录。", verify);
  std::wstring forwarded;
  for (const auto& arg : arguments) forwarded += L" " + QuoteWindows(arg);
  forwarded += L" " + QuoteWindows(std::wstring(kToken) + token);
  // Embedded script: no external cmd/ps1, no UAC, no package writes. Fresh
  // discovery on every invocation follows Store updates instead of pinning paths.
  const std::wstring script =
      L"$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);try {"
      L"$env:PSModulePath=Join-Path $env:SystemRoot 'System32\\WindowsPowerShell\\v1.0\\Modules';"
      L"$p=Get-AppxPackage -Name OpenAI.Codex|Sort-Object Version -Descending|Select-Object -First 1;"
      L"if(!$p){exit 10};"
      L"if($p.PackageFamilyName -cne 'OpenAI.Codex_2p2nqsd0c76g0'){throw 'Unexpected package family'};"
      L"[xml]$m=Get-Content -LiteralPath (Join-Path $p.InstallLocation 'AppxManifest.xml') -Raw;"
      L"$a=@($m.Package.Applications.Application)|Where-Object Id -eq 'App'|Select-Object -First 1;"
      L"if(!$a -or $a.EntryPoint -cne 'Windows.FullTrustApplication'){throw 'Unexpected application entry'};"
      L"$args=" + QuotePowerShell(forwarded) + L"+' \"--codex-context-child='+$p.PackageFullName+'\"';"
      L"Invoke-CommandInDesktopPackage -PackageFamilyName $p.PackageFamilyName -AppId 'App' -Command " +
      QuotePowerShell(executable.wstring()) + L" -Args $args -PreventBreakaway;exit 0"
      L"}catch{[Console]::Error.WriteLine($_.Exception.Message);exit 1}";
  std::wstring system(32768, L'\0');
  UINT n = GetSystemDirectoryW(system.data(), static_cast<UINT>(system.size()));
  if (!n || n >= system.size()) return Failure(root, L"无法定位 Windows PowerShell。", verify);
  system.resize(n);
  const auto powershell = Path(system) / L"WindowsPowerShell" / L"v1.0" / L"powershell.exe";
  std::wstring command = QuoteWindows(powershell.wstring()) +
      L" -NoProfile -NonInteractive -EncodedCommand " + Encode(script);
  SECURITY_ATTRIBUTES attributes{sizeof(SECURITY_ATTRIBUTES), nullptr, TRUE};
  Handle output{CreateFileW((run / L"bootstrap.log").c_str(), GENERIC_WRITE, FILE_SHARE_READ, &attributes,
      CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr)};
  Handle input{CreateFileW(L"NUL", GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, &attributes,
      OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr)};
  if (output.value == INVALID_HANDLE_VALUE || input.value == INVALID_HANDLE_VALUE)
    return Failure(root, L"无法准备启动日志。", verify);
  STARTUPINFOW startup{}; startup.cb = sizeof(startup);
  startup.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES; startup.wShowWindow = SW_HIDE;
  startup.hStdInput = input.value; startup.hStdOutput = output.value; startup.hStdError = output.value;
  PROCESS_INFORMATION information{};
  Log(root, L"context_requested");
  if (!CreateProcessW(powershell.c_str(), command.data(), nullptr, nullptr, TRUE, CREATE_NO_WINDOW,
      nullptr, root.c_str(), &startup, &information))
    return Failure(root, L"无法启动包身份引导程序，Windows 错误 " + std::to_wstring(GetLastError()), verify);
  Handle process{information.hProcess}, thread{information.hThread};
  auto deadline = GetTickCount64() + 45000;
  bool helper_done = false;
  while (GetTickCount64() < deadline) {
    int code = -1;
    { std::ifstream result(run / L"result.txt"); if (result >> code) {
        Log(root, L"context_handoff_result=" + std::to_wstring(code)); return code == 0 ? 0 : 1;
    } }
    if (!helper_done && WaitForSingleObject(process.value, 0) == WAIT_OBJECT_0) {
      DWORD exit_code = 1; GetExitCodeProcess(process.value, &exit_code); helper_done = true;
      if (exit_code == 10) {
        Log(root, L"no_registered_store_package"); return verify ? Verify(root) : -1;
      }
      if (exit_code != 0) return Failure(root, L"Windows 包身份引导失败（退出码 " + std::to_wstring(exit_code) + L"），请检查 " + (run / L"bootstrap.log").wstring(), verify);
    }
    Sleep(100);
  }
  { std::ofstream cancelled(run / L"cancel"); cancelled << "timeout"; }
  return Failure(root, L"包身份交接超时，已有 Codex 未受影响。", verify);
}
}
