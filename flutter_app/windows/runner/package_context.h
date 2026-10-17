#ifndef RUNNER_PACKAGE_CONTEXT_H_
#define RUNNER_PACKAGE_CONTEXT_H_
#include <string>
#include <vector>

namespace package_context {
// -1 means continue to Flutter; otherwise the bootstrap handled this invocation.
int Prepare(const std::vector<std::wstring>& arguments);
std::wstring QuoteWindows(const std::wstring& value);
std::wstring QuotePowerShell(const std::wstring& value);
bool IsCodexIdentity(const std::wstring& package, const std::wstring& app_id);
bool IsInternalArgument(const std::wstring& argument);
}
#endif
