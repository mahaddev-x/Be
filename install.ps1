#!/usr/bin/env pwsh
# BeeHive installer for Windows
# Usage: irm https://mahaddev-x.github.io/beehive/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo    = "mahaddev-x/beehive"
$BinDir  = "$env:USERPROFILE\.beehive\bin"
$BeesDir = "$env:USERPROFILE\.beehive\bees"

$BuiltInBees = @(
    "sentiment-scorer.yaml",
    "text-classifier.yaml",
    "data-extractor.yaml",
    "url-scraper.yaml",
    "file-reviewer.yaml"
)

Write-Host ""
Write-Host "  Installing BeeHive..." -ForegroundColor Cyan

# Fetch latest release metadata
try {
    $Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
} catch {
    Write-Host "  Failed to fetch release info from GitHub." -ForegroundColor Red
    exit 1
}

$Version  = $Release.tag_name
$FileName = "beehive-$Version-bun-windows-x64.zip"
$Asset    = $Release.assets | Where-Object { $_.name -eq $FileName } | Select-Object -First 1

if (-not $Asset) {
    Write-Host "  Could not find asset: $FileName" -ForegroundColor Red
    exit 1
}

Write-Host "  Downloading BeeHive $Version..." -ForegroundColor Cyan

# Create dirs
New-Item -ItemType Directory -Force -Path $BinDir  | Out-Null
New-Item -ItemType Directory -Force -Path $BeesDir | Out-Null

# Download and extract binary
$ZipPath = "$env:TEMP\beehive-install.zip"
Invoke-WebRequest $Asset.browser_download_url -OutFile $ZipPath -UseBasicParsing
Expand-Archive -Path $ZipPath -DestinationPath $BinDir -Force
Remove-Item $ZipPath

# Download built-in bees (skip if already present)
$BaseUrl = "https://raw.githubusercontent.com/$Repo/main/bees"
foreach ($Bee in $BuiltInBees) {
    $Dest = "$BeesDir\$Bee"
    if (-not (Test-Path $Dest)) {
        Invoke-WebRequest "$BaseUrl/$Bee" -OutFile $Dest -UseBasicParsing
    }
}

# Add BinDir to user PATH permanently (if not already there)
$UserPath = [System.Environment]::GetEnvironmentVariable("PATH", "User")
if (-not $UserPath) { $UserPath = "" }
if ($UserPath -notlike "*$BinDir*") {
    [System.Environment]::SetEnvironmentVariable("PATH", "$BinDir;$UserPath", "User")
}

# Refresh current session PATH so beehive works immediately
$env:PATH = "$BinDir;$env:PATH"

Write-Host ""
Write-Host "  BeeHive $Version installed!" -ForegroundColor Green
Write-Host ""
Write-Host "  Run:" -ForegroundColor Yellow
Write-Host "    beehive setup" -ForegroundColor White
Write-Host ""
