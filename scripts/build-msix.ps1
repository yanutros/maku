# Paquet MSIX de Maku pour le Microsoft Store, à partir de la version déjà compilée
# (src-tauri/target/release/plexr.exe, par `npx tauri build --no-bundle`) et de mpv (src-tauri/mpv-win).
# Le Store signe le paquet à la publication : il n'est pas signé ici.
#
#   powershell -ExecutionPolicy Bypass -File scripts/build-msix.ps1
#
# L'identité du paquet (Espace partenaires) est écrite dans src-tauri/msix/AppxManifest.xml ; seule la version
# y est remplie ici, d'après tauri.conf.json.
param(
  [string] $Out = "src-tauri/target/msix"
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

# Version du Store : quatre nombres, le dernier à 0.
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version + ".0"

$layout = Join-Path $Out "layout"
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
New-Item -ItemType Directory -Force "$layout/Assets", "$layout/mpv/mpv" | Out-Null

# Le programme, et mpv à côté, comme dans l'installeur (voir tauri.windows.conf.json).
Copy-Item src-tauri/target/release/plexr.exe $layout
Copy-Item src-tauri/mpv-win/mpv.exe, src-tauri/mpv-win/d3dcompiler_43.dll "$layout/mpv"
Copy-Item src-tauri/mpv-win/mpv/fonts.conf "$layout/mpv/mpv"

# Icônes du Store et de la vignette du menu Démarrer.
foreach ($name in "StoreLogo", "Square44x44Logo", "Square150x150Logo") {
  Copy-Item "src-tauri/icons/$name.png" "$layout/Assets/$name.png"
}

$manifest = (Get-Content src-tauri/msix/AppxManifest.xml -Raw).Replace("{{VERSION}}", $version)
Set-Content "$layout/AppxManifest.xml" $manifest -Encoding utf8

# makeappx vient du SDK Windows (présent sur les machines de GitHub et avec Visual Studio).
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
  Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "makeappx.exe introuvable (SDK Windows)" }

$msix = Join-Path $Out "Maku_${version}_x64.msix"
& $makeappx.FullName pack /o /d $layout /p $msix | Out-Host
if ($LASTEXITCODE) { throw "makeappx a échoué ($LASTEXITCODE)" }
Write-Host "Paquet : $msix"
