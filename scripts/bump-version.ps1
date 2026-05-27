#!/usr/bin/env pwsh
# bump-version.ps1 — Bumps all version references across the Victauri workspace.
#
# Usage:
#   .\scripts\bump-version.ps1 0.5.4
#   .\scripts\bump-version.ps1 0.6.0 -DryRun
#
# What it updates:
#   - Cargo.toml [workspace.package] version
#   - Cargo.lock (via cargo check)
#   - extensions/chrome/manifest.json
#   - extensions/firefox/manifest.json
#   - extensions/chrome/popup/popup.html
#   - extensions/firefox/popup/popup.html
#   - extensions/npm/package.json
#   - editors/vscode/package.json
#   - editors/vscode/package-lock.json
#   - crates/victauri-plugin/src/js_bridge.rs (bridge version constant)
#   - docs/src/getting-started.md (example output)
#   - docs/src/compatibility.md (example output)
#   - CLAUDE.md (Current State header and version ref)
#
# Does NOT update:
#   - CHANGELOG.md (requires human-written release notes)
#   - MIGRATION.md (requires human-written migration guide)
#   - README.md (intentionally version-free)
#   - Test counts (require running tests to get actual numbers)

param(
    [Parameter(Mandatory=$true, Position=0)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$NewVersion,

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$root\Cargo.toml")) {
    $root = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$root\Cargo.toml")) {
    Write-Error "Cannot find Cargo.toml — run from the victauri repo root or scripts/ dir"
    exit 1
}

# Detect current version from Cargo.toml
$cargoToml = Get-Content "$root\Cargo.toml" -Raw
if ($cargoToml -match 'version\s*=\s*"(\d+\.\d+\.\d+)"') {
    $OldVersion = $matches[1]
} else {
    Write-Error "Cannot detect current version from Cargo.toml"
    exit 1
}

if ($OldVersion -eq $NewVersion) {
    Write-Host "Already at version $NewVersion — nothing to do." -ForegroundColor Yellow
    exit 0
}

Write-Host "Bumping $OldVersion -> $NewVersion" -ForegroundColor Cyan

# Helper: replace version in a file
function Update-File {
    param(
        [string]$Path,
        [string]$Pattern,
        [string]$Replacement,
        [string]$Description
    )
    $fullPath = Join-Path $root $Path
    if (-not (Test-Path $fullPath)) {
        Write-Host "  SKIP $Path (not found)" -ForegroundColor Yellow
        return
    }
    $content = Get-Content $fullPath -Raw
    if ($content -match [regex]::Escape($Pattern)) {
        if ($DryRun) {
            Write-Host "  WOULD $Description" -ForegroundColor DarkGray
        } else {
            $content = $content -replace [regex]::Escape($Pattern), $Replacement
            Set-Content $fullPath $content -NoNewline
            Write-Host "  OK    $Description" -ForegroundColor Green
        }
    } else {
        Write-Host "  SKIP  $Description (pattern not found)" -ForegroundColor Yellow
    }
}

# 1. Cargo.toml workspace version
Update-File "Cargo.toml" "version = `"$OldVersion`"" "version = `"$NewVersion`"" "Cargo.toml workspace version"

# 2. Chrome extension manifest
Update-File "extensions\chrome\manifest.json" "`"version`": `"$OldVersion`"" "`"version`": `"$NewVersion`"" "Chrome extension manifest"

# 3. Firefox extension manifest
Update-File "extensions\firefox\manifest.json" "`"version`": `"$OldVersion`"" "`"version`": `"$NewVersion`"" "Firefox extension manifest"

# 4. Chrome popup
Update-File "extensions\chrome\popup\popup.html" "v$OldVersion" "v$NewVersion" "Chrome popup version"

# 5. Firefox popup
Update-File "extensions\firefox\popup\popup.html" "v$OldVersion" "v$NewVersion" "Firefox popup version"

# 6. npm package
Update-File "extensions\npm\package.json" "`"version`": `"$OldVersion`"" "`"version`": `"$NewVersion`"" "npm package version"

# 7. VS Code extension
Update-File "editors\vscode\package.json" "`"version`": `"$OldVersion`"" "`"version`": `"$NewVersion`"" "VS Code package.json"

# 8. VS Code package-lock (both occurrences)
$lockPath = Join-Path $root "editors\vscode\package-lock.json"
if (Test-Path $lockPath) {
    $lockContent = Get-Content $lockPath -Raw
    $oldPattern = "`"version`": `"$OldVersion`""
    $newPattern = "`"version`": `"$NewVersion`""
    if ($lockContent -match [regex]::Escape($oldPattern)) {
        if ($DryRun) {
            Write-Host "  WOULD VS Code package-lock.json" -ForegroundColor DarkGray
        } else {
            $lockContent = $lockContent -replace [regex]::Escape($oldPattern), $newPattern
            Set-Content $lockPath $lockContent -NoNewline
            Write-Host "  OK    VS Code package-lock.json" -ForegroundColor Green
        }
    } else {
        Write-Host "  SKIP  VS Code package-lock.json (pattern not found)" -ForegroundColor Yellow
    }
}

# 9. JS bridge version (hardcoded in bridge init script)
Update-File "crates\victauri-plugin\src\js_bridge.rs" "version: '$OldVersion'" "version: '$NewVersion'" "JS bridge version"

# 10. docs/src/getting-started.md version in example output
Update-File "docs\src\getting-started.md" "`"version`":`"$OldVersion`"" "`"version`":`"$NewVersion`"" "docs getting-started.md example"

# 11. docs/src/compatibility.md bridge_version
Update-File "docs\src\compatibility.md" "`"bridge_version`": `"$OldVersion`"" "`"bridge_version`": `"$NewVersion`"" "docs compatibility.md bridge_version"

# 12. CLAUDE.md Current State version ref
$claudeMd = Join-Path $root "CLAUDE.md"
if (Test-Path $claudeMd) {
    $content = Get-Content $claudeMd -Raw
    $updated = $content -replace [regex]::Escape("v$OldVersion"), "v$NewVersion"
    if ($updated -ne $content) {
        if ($DryRun) {
            Write-Host "  WOULD CLAUDE.md version references" -ForegroundColor DarkGray
        } else {
            Set-Content $claudeMd $updated -NoNewline
            Write-Host "  OK    CLAUDE.md version references" -ForegroundColor Green
        }
    } else {
        Write-Host "  SKIP  CLAUDE.md (no v$OldVersion references)" -ForegroundColor Yellow
    }
}

# 12. Update Cargo.lock via cargo check
if (-not $DryRun) {
    Write-Host "`nUpdating Cargo.lock..." -ForegroundColor Cyan
    Push-Location $root
    try {
        cargo check --workspace 2>&1 | Select-Object -Last 1
    } finally {
        Pop-Location
    }
}

Write-Host "`n--- Version bump complete: $OldVersion -> $NewVersion ---" -ForegroundColor Cyan

if ($DryRun) {
    Write-Host "(dry run — no files were modified)" -ForegroundColor DarkGray
}

Write-Host @"

Remaining manual steps:
  1. Update CHANGELOG.md with release notes
  2. Update MIGRATION.md if there are breaking/behavior changes
  3. Update CLAUDE.md Current State date and new feature descriptions
  4. Run: cargo test --workspace
  5. Run: cargo clippy --workspace --all-targets
  6. Commit, push, publish: cargo publish -p <crate> for each crate
"@ -ForegroundColor DarkYellow
