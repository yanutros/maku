# Version Google Play de Maku pour Android : un .aab (Android App Bundle), signé avec la clé
# d'importation (celle de l'APK, voir src-tauri/gen/android/keystore.properties) ; Google signe ensuite
# l'app lui-même (Play App Signing).
#
#   powershell -ExecutionPolicy Bypass -File scripts/build-play.ps1
#
# Google Play interdit qu'une app se mette à jour en dehors du Play Store : cette version est construite
# avec MAKU_PLAY (la mise à jour intégrée se tait, voir src-tauri/src/update.rs) et sans la permission
# REQUEST_INSTALL_PACKAGES, retirée du manifeste le temps de la construction. L'APK de GitHub garde les deux.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$manifest = "src-tauri/gen/android/app/src/main/AndroidManifest.xml"
# Lu et réécrit en UTF-8 tel quel (Windows PowerShell lirait sinon le fichier dans le codage du système).
$utf8 = New-Object System.Text.UTF8Encoding $false
$original = [IO.File]::ReadAllText((Resolve-Path $manifest), $utf8)
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version

try {
  $play = [regex]::Replace($original, '\s*<!--[^\r\n]*-->\s*<uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES" />', '')
  if ($play -eq $original) { throw "Permission REQUEST_INSTALL_PACKAGES introuvable dans le manifeste" }
  [IO.File]::WriteAllText((Resolve-Path $manifest), $play, $utf8)

  $env:MAKU_PLAY = "1"
  npx tauri android build --aab --target aarch64 --target armv7
  if ($LASTEXITCODE) { throw "La construction a échoué ($LASTEXITCODE)" }
}
finally {
  # Le manifeste de l'APK, tel qu'il était.
  [IO.File]::WriteAllText((Resolve-Path $manifest), $original, $utf8)
  Remove-Item Env:MAKU_PLAY -ErrorAction SilentlyContinue
}

$aab = Get-ChildItem src-tauri/gen/android/app/build/outputs/bundle -Recurse -Filter *.aab |
  Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $aab) { throw "Aucun .aab produit" }
$out = "src-tauri/target/Maku_${version}_play.aab"
Copy-Item $aab.FullName $out -Force
Write-Host "Paquet Google Play : $out"
