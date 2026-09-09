#requires -Version 7.2
<#
.SYNOPSIS
Starts the pinned Luban MCP stdio server for an MCP client.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$lock = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'toolchain.lock.json') -Raw | ConvertFrom-Json
if ($lock.dotnet.source -ne 'system') {
    throw 'This toolchain requires a system-provided dotnet runtime.'
}
$dotnetPath = & (Join-Path $PSScriptRoot 'resolve-dotnet.ps1') -MinimumVersion $lock.dotnet.minimumVersion -Framework $lock.dotnet.framework
$cacheRoot = Join-Path $PSScriptRoot '.cache'
$entryPoints = @{}
foreach ($component in @('luban', 'agent', 'mcp')) {
    $artifact = $lock.$component
    $entryPoint = Join-Path (Join-Path $cacheRoot $artifact.installDirectory) $artifact.entryPoint
    if (-not (Test-Path -LiteralPath $entryPoint -PathType Leaf)) {
        throw "The pinned $($artifact.name) is missing. Run tools/luban/setup.ps1 first."
    }
    $entryPoints[$component] = $entryPoint
}

$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $dotnetPath
$startInfo.UseShellExecute = $false
$startInfo.WorkingDirectory = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
foreach ($argument in @('exec', '--roll-forward', $lock.dotnet.rollForward, $entryPoints.mcp)) {
    $startInfo.ArgumentList.Add([string]$argument)
}
# MCP starts Agent/Luban via dotnet; its children must inherit the same runtime policy.
$startInfo.Environment['DOTNET_ROLL_FORWARD'] = $lock.dotnet.rollForward
$startInfo.Environment['DOTNET_CLI_TELEMETRY_OPTOUT'] = '1'
$startInfo.Environment['PATH'] = (Split-Path -Parent $dotnetPath) + [System.IO.Path]::PathSeparator + $startInfo.Environment['PATH']
$startInfo.Environment['LUBAN_AGENT_DLL'] = $entryPoints.agent
$startInfo.Environment['LUBAN_DLL'] = $entryPoints.luban
# Inherit stdin/stdout/stderr unchanged: stdout belongs exclusively to the MCP protocol.
$process = [System.Diagnostics.Process]::Start($startInfo)
if ($null -eq $process) {
    throw 'Could not start the pinned Luban MCP server.'
}
try {
    $process.WaitForExit()
    $exitCode = $process.ExitCode
} finally {
    $process.Dispose()
}
exit $exitCode
