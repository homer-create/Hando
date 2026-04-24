# Builds a portable ImageOpt folder for Windows that runs without installation.
# Usage (from repo root or anywhere):
#   cd d:\git\ImageOpt\desktop
#   powershell -ExecutionPolicy Bypass -File scripts\make-portable.ps1
#
# Output: desktop\ImageOpt-portable\   (ready to zip and share)

$ErrorActionPreference = "Stop"

# Resolve paths relative to this script
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DesktopDir = Split-Path -Parent $ScriptDir
$RepoRoot = Split-Path -Parent $DesktopDir
$TauriDir = Join-Path $DesktopDir "src-tauri"
$OutDir = Join-Path $DesktopDir "ImageOpt-portable"
$ReleaseDir = Join-Path $TauriDir "target\release"

Write-Host "==> Repo:    $RepoRoot"
Write-Host "==> Output:  $OutDir"

# 1. Build Rust in release mode
Write-Host "`n==> Building Rust (release)..."
Push-Location $TauriDir
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
Pop-Location

# 2. Find the produced executable
$ExeCandidates = @("ImageOpt.exe", "desktop.exe")
$Exe = $null
foreach ($name in $ExeCandidates) {
    $p = Join-Path $ReleaseDir $name
    if (Test-Path $p) { $Exe = $p; break }
}
if (-not $Exe) { throw "Couldn't find ImageOpt.exe or desktop.exe in $ReleaseDir" }
Write-Host "==> Found exe: $Exe"

# 3. Find Node binary
$NodeBin = Join-Path $TauriDir "binaries\node-x86_64-pc-windows-msvc.exe"
if (-not (Test-Path $NodeBin)) { throw "Node binary not found at $NodeBin. Run the fetch step first." }

# 4. Clean output directory
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Path $OutDir | Out-Null

# 5. Copy the main .exe (rename to ImageOpt.exe for branding)
Copy-Item $Exe (Join-Path $OutDir "ImageOpt.exe")

# 6. Copy Node binary (renamed to plain node.exe — matches node_binary() expectation)
Copy-Item $NodeBin (Join-Path $OutDir "node.exe")

# 7. Copy sidecar + shared JS modules
$SrcOut = Join-Path $OutDir "src"
New-Item -ItemType Directory -Path $SrcOut | Out-Null
Copy-Item (Join-Path $RepoRoot "src\sidecar.js") $SrcOut
Copy-Item (Join-Path $RepoRoot "src\encoder.js") $SrcOut
Copy-Item (Join-Path $RepoRoot "src\config.js")  $SrcOut

# 8. Copy Sharp + its native dependencies (the heavy part)
$NodeModulesSrc = Join-Path $RepoRoot "node_modules"
$NodeModulesOut = Join-Path $OutDir "node_modules"
New-Item -ItemType Directory -Path $NodeModulesOut | Out-Null

$SharpPackages = @(
    "sharp",
    "@img\sharp-win32-x64",
    "@img\sharp-libvips-win32-x64",
    "color",
    "color-convert",
    "color-name",
    "color-string",
    "detect-libc",
    "is-arrayish",
    "semver",
    "simple-swizzle"
)

foreach ($pkg in $SharpPackages) {
    $from = Join-Path $NodeModulesSrc $pkg
    $to = Join-Path $NodeModulesOut $pkg
    if (Test-Path $from) {
        Write-Host "==> Copying node_modules\$pkg"
        $parent = Split-Path -Parent $to
        if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent | Out-Null }
        Copy-Item -Recurse -Force $from $to
    } else {
        Write-Warning "Missing node_modules\$pkg (may be optional, continuing)"
    }
}

# 9. Create a README for end users
$ReadmePath = Join-Path $OutDir "README.txt"
@"
ImageOpt — Portable Build
==========================

Double-click ImageOpt.exe to run. No installation required.

Tip: On first launch Windows SmartScreen may warn that the app is
     unrecognised. Click "More info" -> "Run anyway".

Files in this folder (do not delete):
  ImageOpt.exe       the main app
  node.exe           bundled Node.js runtime
  src\               image-processing scripts
  node_modules\      Sharp image library + dependencies

You can move this whole folder anywhere on your computer or put it on
a USB drive.
"@ | Out-File -FilePath $ReadmePath -Encoding utf8

# 10. Report size
$SizeMB = [math]::Round((Get-ChildItem -Recurse $OutDir | Measure-Object -Property Length -Sum).Sum / 1MB, 1)
Write-Host "`n==> Portable build ready at: $OutDir"
Write-Host "==> Total size: $SizeMB MB"
Write-Host "==> To distribute: zip the ImageOpt-portable folder and share."
