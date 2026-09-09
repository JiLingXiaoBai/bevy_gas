#requires -Version 7.2
<#
.SYNOPSIS
Downloads and verifies the pinned Windows x64 Luban toolchain.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $IsWindows -or [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'This toolchain lock currently supports Windows x64 only.'
}

$lock = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'toolchain.lock.json') -Raw | ConvertFrom-Json
if ($lock.dotnet.source -ne 'system') {
    throw 'This toolchain requires a system-provided dotnet runtime.'
}
$dotnetPath = & (Join-Path $PSScriptRoot 'resolve-dotnet.ps1') -MinimumVersion $lock.dotnet.minimumVersion -Framework $lock.dotnet.framework
Write-Host "Using local dotnet: $dotnetPath (runtime >= $($lock.dotnet.minimumVersion), roll-forward $($lock.dotnet.rollForward))."
$cacheRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '.cache'))
New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null

function Get-CachePath {
    param([Parameter(Mandatory)][string]$RelativePath)

    $resolved = [System.IO.Path]::GetFullPath((Join-Path $cacheRoot $RelativePath))
    $prefix = $cacheRoot + [System.IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Path escapes the toolchain cache: $RelativePath"
    }
    return $resolved
}

function Assert-ArchiveHash {
    param([string]$Path, $Artifact)

    $actual = (Get-FileHash -LiteralPath $Path -Algorithm $Artifact.checksum.algorithm).Hash
    if ($actual -ne $Artifact.checksum.value) {
        throw "Checksum mismatch for $($Artifact.archiveName). Expected $($Artifact.checksum.value), got $actual."
    }
}

function Install-PinnedArtifact {
    param($Artifact)

    $archivePath = Get-CachePath "downloads/$($Artifact.archiveName)"
    $installPath = Get-CachePath $Artifact.installDirectory
    $entryPoint = Get-CachePath "$($Artifact.installDirectory)/$($Artifact.entryPoint)"
    New-Item -ItemType Directory -Path (Split-Path -Parent $archivePath) -Force | Out-Null

    if (-not (Test-Path -LiteralPath $archivePath)) {
        $partialPath = $archivePath + '.download'
        Write-Host "Downloading $($Artifact.archiveName)..."
        Invoke-WebRequest -Uri $Artifact.url -OutFile $partialPath -TimeoutSec 240
        Assert-ArchiveHash -Path $partialPath -Artifact $Artifact
        Move-Item -LiteralPath $partialPath -Destination $archivePath
    }
    Assert-ArchiveHash -Path $archivePath -Artifact $Artifact

    if (Test-Path -LiteralPath $installPath) {
        $receiptPath = Join-Path $installPath '.archive-checksum'
        if (-not (Test-Path -LiteralPath $receiptPath) -or
            (Get-Content -LiteralPath $receiptPath -Raw).Trim() -ne $Artifact.checksum.value -or
            -not (Test-Path -LiteralPath $entryPoint)) {
            throw "Incomplete or mismatched installation at $installPath. Move it aside before running setup again."
        }
        Write-Host "Verified cached $($Artifact.name)."
        return
    }

    $stagePath = Get-CachePath ("staging-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $stagePath | Out-Null
    if ($Artifact.format -eq '7z') {
        $extractor = Get-Command 7z.exe -ErrorAction SilentlyContinue
        if ($null -eq $extractor) {
            $extractor = Get-Command 7za.exe -ErrorAction SilentlyContinue
        }
        if ($null -eq $extractor) {
            throw 'Install 7-Zip and add 7z.exe to PATH before running setup.'
        }
        & $extractor.Source x $archivePath "-o$stagePath" -y -bso0 -bsp0
        if ($LASTEXITCODE -ne 0) {
            throw "7-Zip failed with exit code $LASTEXITCODE."
        }
    } elseif ($Artifact.format -eq 'zip') {
        Expand-Archive -LiteralPath $archivePath -DestinationPath $stagePath
    } else {
        throw "Unsupported archive format: $($Artifact.format)"
    }

    $stagedEntryPoint = Get-CachePath ((Split-Path -Leaf $stagePath) + '/' + $Artifact.entryPoint)
    if (-not (Test-Path -LiteralPath $stagedEntryPoint)) {
        throw "Archive does not contain the expected entry point: $($Artifact.entryPoint)"
    }
    Set-Content -LiteralPath (Join-Path $stagePath '.archive-checksum') -Value $Artifact.checksum.value -Encoding ascii

    # Both absolute directory paths were checked against the cache root before moving.
    Move-Item -LiteralPath $stagePath -Destination $installPath
    Write-Host "Installed $($Artifact.name)."
}

Install-PinnedArtifact -Artifact $lock.luban
Install-PinnedArtifact -Artifact $lock.agent
Install-PinnedArtifact -Artifact $lock.mcp

$lubanPath = Get-CachePath "$($lock.luban.installDirectory)/$($lock.luban.entryPoint)"
# Luban 5.0.0 writes version/help to stderr and returns 1 for these informational requests.
$PSNativeCommandUseErrorActionPreference = $false
$versionOutput = & $dotnetPath exec --roll-forward $lock.dotnet.rollForward $lubanPath --version 2>&1
$versionExitCode = $LASTEXITCODE
$expectedVersion = "Luban $($lock.luban.version)+$($lock.luban.sourceCommit)"
$actualVersion = ($versionOutput -join [Environment]::NewLine).Trim()
if ($versionExitCode -notin @(0, 1) -or $actualVersion -ne $expectedVersion) {
    throw "Luban version verification failed (exit $versionExitCode): $actualVersion"
}
$agentPath = Get-CachePath "$($lock.agent.installDirectory)/$($lock.agent.entryPoint)"
$expectedAgentVersion = "$($lock.agent.version)+$($lock.agent.sourceCommit)"
$actualAgentVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($agentPath).ProductVersion
if ($actualAgentVersion -ne $expectedAgentVersion) {
    throw "Luban.Agent version verification failed: expected $expectedAgentVersion, got $actualAgentVersion."
}
$capabilitiesOutput = & $dotnetPath exec --roll-forward $lock.dotnet.rollForward $agentPath capabilities
$capabilitiesExitCode = $LASTEXITCODE
if ($capabilitiesExitCode -ne 0) {
    throw "Luban.Agent capabilities check failed with exit code $capabilitiesExitCode."
}
$capabilities = ($capabilitiesOutput -join [Environment]::NewLine) | ConvertFrom-Json
if ($capabilities.ok -ne $true -or $capabilities.exitCode -ne 0 -or
    $capabilities.command -ne 'capabilities' -or $capabilities.result.tool -ne 'Luban.Agent') {
    throw 'Luban.Agent returned an unexpected capabilities response.'
}
foreach ($mode in @('capabilities', 'list-tables', 'describe', 'schema', 'validate')) {
    if ($mode -notin $capabilities.result.modes) {
        throw "Luban.Agent is missing the expected mode: $mode"
    }
}
$mcpPath = Get-CachePath "$($lock.mcp.installDirectory)/$($lock.mcp.entryPoint)"
# Upstream v5.0.0 leaves the MCP assembly version at 1.0.0; its release is pinned by archive hash.
$expectedMcpVersion = "$($lock.mcp.assemblyVersion)+$($lock.mcp.sourceCommit)"
$actualMcpVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($mcpPath).ProductVersion
if ($actualMcpVersion -ne $expectedMcpVersion) {
    throw "Luban.Mcp version verification failed: expected $expectedMcpVersion, got $actualMcpVersion."
}
Write-Host "Luban $($lock.luban.version), Luban.Agent $($lock.agent.version) and Luban.Mcp $($lock.mcp.version) are ready with local .NET >= $($lock.dotnet.minimumVersion)."
Write-Host "Run: pwsh -File tools/luban/run.ps1 --help"
Write-Host "MCP: project configuration is in .codex/config.toml; restart the MCP connection after setup."
