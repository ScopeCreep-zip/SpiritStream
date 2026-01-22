# SpiritStream Setup Script for Windows (PowerShell)
# Installs all prerequisites for building and running SpiritStream

# Note: Using Write-Host with -ForegroundColor instead of ANSI codes for compatibility

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
        # Download rustup-init.exe for Windows
        $RustupPath = "$env:TEMP\rustup-init.exe"
        try {
            Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $RustupPath -UseBasicParsing
            # Run installer with -y for unattended install
            Start-Process -FilePath $RustupPath -ArgumentList "-y" -Wait -NoNewWindow
            $Env:Path += ";$env:USERPROFILE\.cargo\bin"
            Print-Step "Rust installed"
        } catch {
            Print-Warning "Failed to download Rust installer. Please install manually from https://rustup.rs"
        } finally {
            if (Test-Path $RustupPath) { Remove-Item $RustupPath -Force }
        }
    }
}

function Check-Node {
    if (Get-Command node -ErrorAction SilentlyContinue) {
        $NodeVersionStr = node --version
        Print-Step "Node.js found: $NodeVersionStr"

        # Extract major.minor version (e.g., "v22.12.0" -> 22, 12)
        if ($NodeVersionStr -match "v(\d+)\.(\d+)") {
            $Major = [int]$Matches[1]
            $Minor = [int]$Matches[2]

            # Vite requires Node.js 20.19+ or 22.12+
            # Odd-numbered versions (21, 23, etc.) are not LTS and may have compatibility issues
            $IsValid = ($Major -eq 20 -and $Minor -ge 19) -or ($Major -ge 22 -and $Minor -ge 12)
            $IsOddVersion = ($Major % 2) -eq 1

            if ($IsOddVersion) {
                Print-Warning "Node.js $Major.x is an odd-numbered (non-LTS) release which may cause issues."
                Print-Warning "Recommended: Install Node.js 22.x LTS from https://nodejs.org/"
                return $false
            } elseif (-not $IsValid) {
                Print-Warning "Node.js version $Major.$Minor is too old. Vite requires 20.19+ or 22.12+"
                Print-Warning "Please upgrade Node.js from https://nodejs.org/"
                return $false
            }
            return $true
        }
        Print-Warning "Could not parse Node.js version"
        return $false
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

function Install-Pnpm-Dependencies {
    if (Test-Path "package.json") {
        if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
            Print-Warning "pnpm not found. Install with: corepack prepare pnpm@9.15.0 --activate (or npm install -g pnpm@9.15.0)"
            return
        }
        Print-Step "Installing pnpm dependencies..."
        pnpm install
        Print-Step "pnpm dependencies installed"
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
        Print-Error "Node.js 20.19+ or 22.x LTS is required. Please install from https://nodejs.org/ (LTS version recommended) and rerun this script."
    }

    # Step 3: Install FFmpeg
    Print-Step "Installing FFmpeg..."
    Install-FFmpeg

    # Step 4: Install pnpm dependencies
    Print-Step "Installing pnpm dependencies..."
    Install-Pnpm-Dependencies

    # Done
    Write-Host "" -ForegroundColor Green
    Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Green
    Write-Host "  Setup Complete!" -ForegroundColor Green
    Write-Host "══════════════════════════════════════════════════════════════" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Green
    Write-Host "  1. Restart your terminal (to load Rust environment)" -ForegroundColor Green
    Write-Host "  2. Run: pnpm run dev    (development mode)" -ForegroundColor Green
    Write-Host "  3. Run: pnpm run build  (production build)" -ForegroundColor Green
    Write-Host ""
}

# Run main
Main
