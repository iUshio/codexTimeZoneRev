$ErrorActionPreference = 'Stop'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (-not (Test-Path $compiler)) { $compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework/v4.0.30319/csc.exe' }
$output = Join-Path $PSScriptRoot '..\src-tauri\resources\CodexTimeZoneBackend.exe'
New-Item -ItemType Directory -Path (Split-Path $output) -Force | Out-Null
& $compiler /nologo /target:exe /optimize+ /main:Backend /out:$output /reference:System.Web.Extensions.dll /reference:System.Xml.Linq.dll /reference:Microsoft.CSharp.dll (Join-Path $PSScriptRoot 'Backend.cs')
if ($LASTEXITCODE -ne 0) { throw 'C# 后端构建失败' }
Write-Output "已生成 $output"
