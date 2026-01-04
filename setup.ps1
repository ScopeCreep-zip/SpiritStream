# SpiritStream Setup Script for Windows (PowerShell)
# Installs all prerequisites for building and running SpiritStream

# Colors for output
$Red = "`e[31m"
$Green = "`e[32m"
$Yellow = "`e[33m"
$Blue = "`e[34m"
$Reset = "`e[0m"

function Print-Header {
    Write-Host "" -ForegroundColor Blue
    Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Blue
    Write-Host "  SpiritStream Setup" -ForegroundColor Blue
    Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Blue
    Write-Host ""
}

function Print-Step {
    param ([string]$Message)
    Write-Host "[STEP] $Message" -ForegroundColor Green
}

function Print-Warning {
    param ([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

function Print-Error {
    param ([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
    exit 1
}

function Install-Rust {
    if (Get-Command rustc -ErrorAction SilentlyContinue) {
        $RustVersion = rustc --version
        Print-Step "Rust already installed: $RustVersion"
    } else {
        Print-Step "Installing Rust..."
        Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://sh.rustup.rs')) -ArgumentList "-y"
        $Env:Path += ";$HOME\.cargo\bin"
        Print-Step "Rust installed"
    }
}

function Check-Node {
    if (Get-Command node -ErrorAction SilentlyContinue) {
        $NodeVersion = node --version
        Print-Step "Node.js already installed: $NodeVersion"
        return $true
    } else {
        Print-Warning "Node.js not found"
        return $false
    }
}

function Install-FFmpeg {
    if (Get-Command ffmpeg -ErrorAction SilentlyContinue) {
        $FFmpegVersion = ffmpeg -version | Select-String -Pattern "ffmpeg version"
        Print-Step "FFmpeg already installed: $FFmpegVersion"
    } else {
        Print-Step "Installing FFmpeg..."
        if (Get-Command winget -ErrorAction SilentlyContinue) {
            winget install ffmpeg
        } else {
            Print-Warning "Please install FFmpeg manually from https://ffmpeg.org/download.html"
        }
    }
}

function Install-Npm-Dependencies {
    if (Test-Path "package.json") {
        Print-Step "Installing npm dependencies..."
        npm install
        Print-Step "npm dependencies installed"
    } else {
        Print-Warning "package.json not found. Run this script from the project root."
    }
}

# Main setup function
function Main {
    Print-Header

    # Step 1: Install Rust
    Print-Step "Installing Rust..."
    Install-Rust

    # Step 2: Check Node.js
    Print-Step "Checking Node.js..."
    if (-not (Check-Node)) {
        Print-Error "Node.js is required but not installed. Please install Node.js 18+ from https://nodejs.org/ and rerun this script."
    }

    # Step 3: Install FFmpeg
    Print-Step "Installing FFmpeg..."
    Install-FFmpeg

    # Step 4: Install npm dependencies
    Print-Step "Installing npm dependencies..."
    Install-Npm-Dependencies

    # Done
    Write-Host "" -ForegroundColor Green
    Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Green
    Write-Host "  Setup Complete!" -ForegroundColor Green
    Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Green
    Write-Host "  1. Restart your terminal (to load Rust environment)" -ForegroundColor Green
    Write-Host "  2. Run: npm run dev    (development mode)" -ForegroundColor Green
    Write-Host "  3. Run: npm run build  (production build)" -ForegroundColor Green
    Write-Host ""
}

# Run main
Main