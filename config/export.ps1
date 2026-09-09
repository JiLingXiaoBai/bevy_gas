#requires -Version 7.2
<#
.SYNOPSIS
Exports this configuration project to Rust code and binary data using the pinned Luban toolchain.
.DESCRIPTION
Resolves all paths relative to this script, validates input and output locations,
and forwards the generator exit code. Run tools/luban/setup.ps1 before the first export.
.OUTPUTS
Generated cfg and macros crates under config/generated, and binary tables under config/bin.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$configRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
$repositoryRoot = Split-Path -Parent $configRoot
$runnerPath = Join-Path $repositoryRoot 'tools/luban/run.ps1'
$configurationPath = Join-Path $configRoot 'luban.conf'
$codeOutputPath = Join-Path $configRoot 'generated'
$dataOutputPath = Join-Path $configRoot 'bin'

foreach ($inputPath in @($runnerPath, $configurationPath)) {
    if (-not (Test-Path -LiteralPath $inputPath -PathType Leaf)) {
        throw "Required export file is missing: $inputPath"
    }
}

# Luban owns these output directories and may remove previous generated files.
foreach ($directoryPath in @($configRoot, $codeOutputPath, $dataOutputPath)) {
    if (Test-Path -LiteralPath $directoryPath) {
        $directory = Get-Item -LiteralPath $directoryPath -Force
        if (-not $directory.PSIsContainer -or ($directory.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
            throw "The export requires a regular directory: $directoryPath"
        }
    }
}

$generatorArguments = @(
    '--conf', $configurationPath,
    '-t', 'all',
    '-c', 'rust-bin',
    '-d', 'bin',
    '--strict',
    '-x', "outputCodeDir=$codeOutputPath",
    '-x', "outputDataDir=$dataOutputPath"
)
& $runnerPath @generatorArguments
exit $LASTEXITCODE
