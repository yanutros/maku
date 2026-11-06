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

# Le programme emballé doit être celui de cette version : un plexr.exe resté d'une compilation précédente
# donnerait un paquet au bon numéro mais à l'ancien contenu (le Store afficherait une version, l'application une autre).
$exe = "src-tauri/target/release/plexr.exe"
if (-not (Test-Path $exe)) { throw "$exe introuvable : lance d'abord npx tauri build --no-bundle" }
$exeVersion = (Get-Item $exe).VersionInfo.ProductVersion
if ("$exeVersion.0" -ne $version) {
  throw "$exe est en version $exeVersion, pas en $version : lance d'abord npx tauri build --no-bundle"
}

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

# Barre des tâches, menu Démarrer, Alt+Tab : sans ces variantes « unplated », Windows pose l'icône, réduite,
# sur une plaque de couleur. Avec elles, elle s'affiche en entier, comme celle de l'installeur classique.
Add-Type -AssemblyName System.Drawing
$source = [System.Drawing.Image]::FromFile((Resolve-Path "src-tauri/icons/icon.png"))
$assets = (Resolve-Path "$layout/Assets").Path
try {
  foreach ($size in 16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.DrawImage($source, 0, 0, $size, $size)
    $g.Dispose()
    foreach ($variant in "targetsize-$size", "targetsize-$($size)_altform-unplated", "targetsize-$($size)_altform-lightunplated") {
      $bitmap.Save((Join-Path $assets "Square44x44Logo.$variant.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    }
    $bitmap.Dispose()
  }
} finally {
  $source.Dispose()
}

$manifest = (Get-Content src-tauri/msix/AppxManifest.xml -Raw).Replace("{{VERSION}}", $version)
Set-Content "$layout/AppxManifest.xml" $manifest -Encoding utf8

# makeappx vient du SDK Windows (présent sur les machines de GitHub et avec Visual Studio).
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
  Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "makeappx.exe introuvable (SDK Windows)" }

# Index des ressources (resources.pri) : c'est lui qui fait choisir à Windows la bonne variante d'icône.
$makepri = Join-Path $makeappx.DirectoryName "makepri.exe"
$priconfig = Join-Path $Out "priconfig.xml"
& $makepri createconfig /cf $priconfig /dq "fr-FR_en-US_es-ES_de-DE_nl-NL" /pv 10.0.0 /o | Out-Host
if ($LASTEXITCODE) { throw "makepri createconfig a échoué ($LASTEXITCODE)" }
& $makepri new /pr $layout /cf $priconfig /mn "$layout/AppxManifest.xml" /of "$layout/resources.pri" /o | Out-Host
if ($LASTEXITCODE) { throw "makepri new a échoué ($LASTEXITCODE)" }

$msix = Join-Path $Out "Maku_${version}_x64.msix"
& $makeappx.FullName pack /o /d $layout /p $msix | Out-Host
if ($LASTEXITCODE) { throw "makeappx a échoué ($LASTEXITCODE)" }
Write-Host "Paquet : $msix"
