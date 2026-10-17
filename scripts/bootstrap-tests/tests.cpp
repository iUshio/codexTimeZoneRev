#include <windows.h>
#include <shellapi.h>
#include <iostream>
#include "../../flutter_app/windows/runner/package_context.h"
int main() {
  using namespace package_context;
  const std::vector<std::wstring> values = {
    L"", L"plain", L"two words", L"C:\\中文 目录\\", L"a\"b", L"a\\\"b",
    L"' ; $env:TEMP $(whoami) ` test", L"--solid-background", L"\\\\", L"\n\t"
  };
  std::wstring command = L"test.exe";
  for (const auto& value : values) command += L" " + QuoteWindows(value);
  int argc = 0;
  wchar_t** argv = CommandLineToArgvW(command.c_str(), &argc);
  if (!argv || argc != static_cast<int>(values.size()) + 1) return 1;
  for (size_t i = 0; i < values.size(); ++i) if (values[i] != argv[i + 1]) return 2;
  LocalFree(argv);
  if (QuotePowerShell(L"C:\\a'b $()") != L"'C:\\a''b $()'") return 3;
  const auto package = L"OpenAI.Codex_26.1007.9446.0_x64__2p2nqsd0c76g0";
  const auto app = L"OpenAI.Codex_2p2nqsd0c76g0!App";
  if (!IsCodexIdentity(package, app)) return 4;
  if (IsCodexIdentity(package, L"wrong") || IsCodexIdentity(L"", app) ||
      IsCodexIdentity(L"OpenAI.Codex_1.0_x64__other", app)) return 5;
  if (!IsInternalArgument(L"--codex-context-child=x") ||
      !IsInternalArgument(L"--codex-context-token=x") ||
      IsInternalArgument(L"--solid-background")) return 6;
  std::cout << "Bootstrap quoting, identity and internal-argument contracts passed\n";
  return 0;
}
