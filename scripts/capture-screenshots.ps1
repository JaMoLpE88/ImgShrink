param(
    [string]$Chrome = "C:\Program Files\Google\Chrome\Application\chrome.exe"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dir = Join-Path $root "docs\images"
if (-not (Test-Path $Chrome)) { throw "No se encontró Chrome en $Chrome" }

$profile = Join-Path $env:TEMP "imgshrink-gallery-chrome"
$pages = Get-ChildItem $dir -Filter *.html
if (-not $pages) { throw "No hay HTML en $dir. Ejecuta cargo run -p imgshrink-tui --example gallery" }

foreach ($page in $pages) {
    $png = [System.IO.Path]::ChangeExtension($page.FullName, ".png")
    $uri = "file:///" + ($page.FullName -replace '\\', '/')
    Write-Host "Capturando $($page.Name) -> $([System.IO.Path]::GetFileName($png))"
    & $Chrome --headless=new --disable-gpu "--user-data-dir=$profile" --hide-scrollbars --no-first-run --virtual-time-budget=1500 --force-device-scale-factor=1.5 --window-size=1200,760 "--screenshot=$png" $uri | Out-Null
    if (-not (Test-Path $png)) { throw "Chrome no generó $png" }
    Remove-Item $page.FullName
}
Write-Host "Capturas generadas en $dir"
