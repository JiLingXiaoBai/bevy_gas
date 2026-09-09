#requires -Version 7.2
<#
.SYNOPSIS
Resolves dotnet from PATH and checks for a local runtime meeting the minimum version.
.PARAMETER MinimumVersion
The minimum supported stable runtime version.
.PARAMETER Framework
The framework name reported by dotnet --list-runtimes.
.OUTPUTS
The absolute path to the local dotnet executable.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][version]$MinimumVersion,
    [Parameter(Mandatory)][string]$Framework
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

$dotnetCommand = Get-Command dotnet.exe -CommandType Application -ErrorAction SilentlyContinue |
    Select-Object -First 1
if ($null -eq $dotnetCommand) {
    throw "dotnet.exe was not found in PATH. Install $Framework $MinimumVersion or later and make dotnet available in PATH."
}

$runtimeLines = & $dotnetCommand.Source --list-runtimes 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "Could not list runtimes using $($dotnetCommand.Source): $runtimeLines"
}
$runtimePattern = '^' + [regex]::Escape($Framework) + '\s+(?<Version>\d+\.\d+\.\d+)\s+\['
foreach ($line in $runtimeLines) {
    if ($line.ToString() -match $runtimePattern) {
        $runtimeVersion = $null
        if ([version]::TryParse($Matches.Version, [ref]$runtimeVersion) -and $runtimeVersion -ge $MinimumVersion) {
            return $dotnetCommand.Source
        }
    }
}
throw "$Framework $MinimumVersion or later is not installed for $($dotnetCommand.Source). Install a supported runtime or select the correct dotnet in PATH."
