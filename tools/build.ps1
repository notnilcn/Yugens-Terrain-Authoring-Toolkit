# Builds the GDExtension libraries and installs them where the .gdextension files expect them
# (same layout as tools/build.sh):
#   marching_squares_terrain -> addons/MarchingSquaresTerrain/bin,
#   demo-only mst_demo -> demo/bin,
#   demo-only phantom_camera_rust (GPL-3.0) -> addons/phantom_camera_rust/bin.
# The addon crate builds from the addon submodule's rust/ workspace; the demo crates build from
# the parent repo's rust/ workspace. Both share rust/target.
# Debug libraries are installed as <lib>.dll (editor, debug exports), release ones as
# <lib>_release.dll (release exports). Binaries are not committed (see .gitignore).
# Usage: powershell -File tools/build.ps1 [-Release | -Ship | -All] [-NoDemo]
#   (default)  debug libraries
#   -Release   release libraries only (the editor still loads the debug ones)
#   -Ship      release libraries, also installed under the plain name (no debug libraries)
#   -All       debug and release libraries
#   -NoDemo    only the MarchingSquaresTerrain addon
param([switch]$Release, [switch]$Ship, [switch]$All, [switch]$NoDemo)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$addonRust = "$root/addons/MarchingSquaresTerrain/rust"
$demoRust = "$root/rust"
$env:CARGO_TARGET_DIR = "$root/rust/target"
$profiles = if ($Ship) { @("release") } elseif ($All) { @("debug", "release") } elseif ($Release) { @("release") } else { @("debug") }
function Assert-Addon($file, $label) {
    if (-not (Test-Path "$root/$file")) {
        throw "$file is missing ($label); run: git submodule update --init --recursive"
    }
}
Assert-Addon "addons/MarchingSquaresTerrain/marching_squares_terrain.gdextension" "the addon is a git submodule"
Assert-Addon "addons/MarchingSquaresTerrain/rust/Cargo.toml" "the addon ships its Rust sources"
if (-not $NoDemo) { Assert-Addon "addons/phantom_camera_rust/phantom_camera_rust.gdextension" "the demo dependency is a git submodule" }
function Install-Lib($profile, $name, $dest, $AlsoPlain = $false) {
    $suffix = if ($profile -eq "release") { "_release" } else { "" }
    New-Item -ItemType Directory -Force $dest | Out-Null
    # Remove first so a hardlinked plain-name copy from an earlier -Ship build is unlinked.
    Remove-Item "$dest/$name$suffix.dll" -Force -ErrorAction SilentlyContinue
    Copy-Item "$root/rust/target/$profile/$name.dll" "$dest/$name$suffix.dll" -Force
    Write-Host "installed $profile $dest/$name$suffix.dll"
    if ($AlsoPlain -and $suffix -ne "") {
        # Prefer a hardlink so a shipped tree carries each library once; copy if unsupported.
        Remove-Item "$dest/$name.dll" -Force -ErrorAction SilentlyContinue
        try {
            New-Item -ItemType HardLink -Path "$dest/$name.dll" -Target "$dest/$name$suffix.dll" -ErrorAction Stop | Out-Null
            Write-Host "installed ship $dest/$name.dll (hardlink, optimized, no debug info)"
        } catch {
            Copy-Item "$dest/$name$suffix.dll" "$dest/$name.dll" -Force
            Write-Host "installed ship $dest/$name.dll (optimized, no debug info)"
        }
    }
}
foreach ($profile in $profiles) {
    # Spell the flag out instead of splatting it: Windows PowerShell 5.1 turns a splatted
    # "--release" into "- - r e l e a s e" for native commands, which cargo rejects.
    Push-Location $addonRust
    try {
        if ($profile -eq "release") { cargo build -p mst --release } else { cargo build -p mst }
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    } finally { Pop-Location }
    Install-Lib $profile "marching_squares_terrain" "$root/addons/MarchingSquaresTerrain/bin" $Ship
    if (-not $NoDemo) {
        Push-Location $demoRust
        try {
            if ($profile -eq "release") {
                cargo build -p mst_demo -p phantom-camera-rust --release
            } else {
                cargo build -p mst_demo -p phantom-camera-rust
            }
            if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
        } finally { Pop-Location }
        Install-Lib $profile "mst_demo" "$root/demo/bin" $Ship
        Install-Lib $profile "phantom_camera_rust" "$root/addons/phantom_camera_rust/bin" $Ship
    }
}
