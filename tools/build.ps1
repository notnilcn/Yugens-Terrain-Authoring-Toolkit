# Builds the GDExtension libraries and installs them where the .gdextension files expect them
# (same layout as tools/build.sh): marching_squares_terrain -> addons/MarchingSquaresTerrain/bin,
# demo-only mst_demo -> demo/bin, demo-only phantom_camera_rust (GPL-3.0) -> addons/phantom_camera_rust/bin.
# Debug libraries are installed as <lib>.dll (editor, debug exports), release ones as
# <lib>_release.dll (release exports). Binaries are not committed (see .gitignore).
# Usage: powershell -File tools/build.ps1 [-Release | -All] [-NoDemo]
param([switch]$Release, [switch]$All, [switch]$NoDemo)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$pkgs = @("-p", "mst")
if (-not $NoDemo) { $pkgs += @("-p", "mst_demo", "-p", "phantom-camera-rust") }
$profiles = if ($All) { @("debug", "release") } elseif ($Release) { @("release") } else { @("debug") }
function Install-Lib($profile, $name, $dest) {
    $suffix = if ($profile -eq "release") { "_release" } else { "" }
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item "$root/rust/target/$profile/$name.dll" "$dest/$name$suffix.dll" -Force
    Write-Host "installed $profile $dest/$name$suffix.dll"
}
foreach ($profile in $profiles) {
    Push-Location "$root/rust"
    try {
        if ($profile -eq "release") { cargo build @pkgs --release } else { cargo build @pkgs }
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    } finally { Pop-Location }
    Install-Lib $profile "marching_squares_terrain" "$root/addons/MarchingSquaresTerrain/bin"
    if (-not $NoDemo) {
        Install-Lib $profile "mst_demo" "$root/demo/bin"
        Install-Lib $profile "phantom_camera_rust" "$root/addons/phantom_camera_rust/bin"
    }
}
