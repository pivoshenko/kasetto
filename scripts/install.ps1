#!/usr/bin/env pwsh
# kasetto installer for Windows
# https://github.com/pivoshenko/kasetto
#
# Usage:
#   powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.ps1 | iex"
#
# Environment variables:
#   KASETTO_VERSION     - version tag to install (default: latest release)
#   KASETTO_INSTALL_DIR - installation directory (default: %USERPROFILE%\.local\bin)

$ErrorActionPreference = "Stop"

$Repo = "pivoshenko/kasetto"

function Main {
    $arch = Get-Arch
    $version = if ($env:KASETTO_VERSION) { $env:KASETTO_VERSION } else { Get-LatestVersion }
    $installDir = if ($env:KASETTO_INSTALL_DIR) { $env:KASETTO_INSTALL_DIR } else { Join-Path $env:USERPROFILE ".local\bin" }

    $target = Get-Target $arch
    $artifact = "kasetto-${target}.zip"

    Log "installing kasetto $version ($target)"

    $tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        $url = "https://github.com/$Repo/releases/download/$version/$artifact"
        $checksumsUrl = "https://github.com/$Repo/releases/download/$version/checksums.txt"

        $archivePath = Join-Path $tmpDir $artifact
        $checksumsPath = Join-Path $tmpDir "checksums.txt"

        Log "downloading $url"
        Invoke-WebRequest -Uri $url -OutFile $archivePath -UseBasicParsing
        Invoke-WebRequest -Uri $checksumsUrl -OutFile $checksumsPath -UseBasicParsing

        Log "verifying checksum"
        Test-Checksum $archivePath $checksumsPath $artifact

        Log "extracting"
        Expand-Archive -Path $archivePath -DestinationPath $tmpDir -Force

        if (-not (Test-Path $installDir)) {
            New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        }

        $dest = Join-Path $installDir "kasetto.exe"
        $kstDest = Join-Path $installDir "kst.exe"

        Copy-Item (Join-Path $tmpDir "kasetto.exe") $dest -Force
        Copy-Item (Join-Path $tmpDir "kst.exe") $kstDest -Force

        Log "installed kasetto to $dest"
        Log "installed kst to $kstDest"

        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($userPath -notlike "*$installDir*") {
            [Environment]::SetEnvironmentVariable("Path", "$installDir;$userPath", "User")
            Log "added $installDir to user PATH"
            Warn "restart your terminal for PATH changes to take effect"
        }

        Log "run 'kasetto --help' to get started"
    }
    finally {
        Remove-Item $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Get-Arch {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        default { Err "unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
    }
}

function Get-Target($arch) {
    return "${arch}-pc-windows-msvc"
}

function Get-LatestVersion {
    $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    if (-not $response.tag_name) {
        Err "could not determine latest version; set KASETTO_VERSION explicitly"
    }
    return $response.tag_name
}

function Test-Checksum($file, $checksumsFile, $artifact) {
    $checksums = Get-Content $checksumsFile
    $line = $checksums | Where-Object { $_ -match [regex]::Escape($artifact) }

    if (-not $line) {
        Warn "checksum not found for $artifact; skipping verification"
        return
    }

    $expected = ($line -split '\s+')[0]
    $actual = (Get-FileHash $file -Algorithm SHA256).Hash.ToLower()

    if ($actual -ne $expected) {
        Err "checksum mismatch: expected $expected, got $actual"
    }
}

function Log($msg) {
    Write-Host "info " -ForegroundColor Green -NoNewline
    Write-Host $msg
}

function Warn($msg) {
    Write-Host "warn " -ForegroundColor Yellow -NoNewline
    Write-Host $msg
}

function Err($msg) {
    Write-Host "error " -ForegroundColor Red -NoNewline
    Write-Host $msg
    exit 1
}

Main
