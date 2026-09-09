#requires -Version 7.2
<#
.SYNOPSIS
Runs the pinned Luban generator and forwards its arguments and exit code.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$lock = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'toolchain.lock.json') -Raw | ConvertFrom-Json
$cacheRoot = Join-Path $PSScriptRoot '.cache'
if ($lock.dotnet.source -ne 'system') {
    throw 'This toolchain requires a system-provided dotnet runtime.'
}
$dotnetPath = & (Join-Path $PSScriptRoot 'resolve-dotnet.ps1') -MinimumVersion $lock.dotnet.minimumVersion -Framework $lock.dotnet.framework
$lubanPath = Join-Path (Join-Path $cacheRoot $lock.luban.installDirectory) $lock.luban.entryPoint
if (-not (Test-Path -LiteralPath $lubanPath)) {
    throw 'The pinned Luban generator is missing. Run tools/luban/setup.ps1 first.'
}

$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $dotnetPath
$startInfo.UseShellExecute = $false
$startInfo.WorkingDirectory = (Get-Location).Path
foreach ($argument in @('exec', '--roll-forward', $lock.dotnet.rollForward, $lubanPath) + $args) {
    $startInfo.ArgumentList.Add([string]$argument)
}
$startInfo.Environment['DOTNET_CLI_TELEMETRY_OPTOUT'] = '1'
$process = [System.Diagnostics.Process]::Start($startInfo)
if ($null -eq $process) {
    throw 'Could not start the pinned Luban generator.'
}
try {
    $process.WaitForExit()
    $exitCode = $process.ExitCode
} finally {
    $process.Dispose()
}
exit $exitCode
