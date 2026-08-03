[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [string]$Binary,
    [Parameter(Mandatory = $true)]
    [string]$Output
)

$ErrorActionPreference = "Stop"

function Find-WixTool([string]$Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $wixRoot = "${env:ProgramFiles(x86)}\WiX Toolset *\bin\$Name"
    $candidate = Get-ChildItem $wixRoot -ErrorAction SilentlyContinue |
        Sort-Object FullName |
        Select-Object -Last 1
    if ($null -ne $candidate) {
        return $candidate.FullName
    }

    throw "$Name was not found; install the WiX Toolset before packaging"
}

$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$payloadDir = Join-Path $env:RUNNER_TEMP "nexac-msi-payload"
$objDir = Join-Path $env:RUNNER_TEMP "nexac-msi-obj"
$candlePath = Find-WixTool "candle.exe"
$lightPath = Find-WixTool "light.exe"
$wixBin = Split-Path -Parent $candlePath
$wixUiExtension = Join-Path $wixBin "WixUIExtension.dll"

Remove-Item -Recurse -Force $payloadDir, $objDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $payloadDir, $objDir | Out-Null
Copy-Item $binaryPath (Join-Path $payloadDir "nexac.exe")
Copy-Item (Join-Path $repoRoot "README.md") (Join-Path $payloadDir "README.md")
Copy-Item (Join-Path $repoRoot "LICENSE") (Join-Path $payloadDir "LICENSE")

$wxsPath = Join-Path $repoRoot "packaging\windows\nexac.wxs"
$wixObject = Join-Path $objDir "nexac.wixobj"
$outputPath = [System.IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force (Split-Path -Parent $outputPath) | Out-Null

& $candlePath "-dSourceDir=$payloadDir" "-dVersion=$Version" "-out" $wixObject $wxsPath
if ($LASTEXITCODE -ne 0) {
    throw "WiX candle failed with exit code $LASTEXITCODE"
}

& $lightPath "-ext" $wixUiExtension "-out" $outputPath $wixObject
if ($LASTEXITCODE -ne 0) {
    throw "WiX light failed with exit code $LASTEXITCODE"
}
