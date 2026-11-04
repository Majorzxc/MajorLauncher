# Сменить версию лаунчера во всех местах сразу: Cargo.toml (ядро, mlcli, окно),
# tauri.conf.json (установщик), package.json и package-lock.json (интерфейс).
# Сборка выпуска (.github/workflows/release.yml) проверяет, что они совпадают с тегом.
#
#   powershell -ExecutionPolicy Bypass -File scripts/set-version.ps1 1.0.0
param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"
if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?$') {
  throw "«$Version» — не номер версии: нужно 1.2.3 или 1.2.3-beta.1"
}
$root = Split-Path $PSScriptRoot -Parent
$utf8 = New-Object System.Text.UTF8Encoding($false)

function Replace-First([string]$path, [string]$pattern, [string]$replacement) {
  $text = [IO.File]::ReadAllText($path)
  $re = [regex]$pattern
  if (-not $re.IsMatch($text)) { throw "в $path не нашлось $pattern" }
  [IO.File]::WriteAllText($path, $re.Replace($text, $replacement, 1), $utf8)
}

Replace-First "$root\Cargo.toml" '(?m)^version = "[^"]+"' "version = `"$Version`""
Replace-First "$root\app\src-tauri\tauri.conf.json" '"version": "[^"]+"' "`"version`": `"$Version`""
Replace-First "$root\app\package.json" '"version": "[^"]+"' "`"version`": `"$Version`""
# В package-lock.json версия пакета записана дважды: в корне и в packages[""].
$lock = "$root\app\package-lock.json"
$text = [IO.File]::ReadAllText($lock)
$text = ([regex]'("name": "majorlauncher-app",\s*"version": )"[^"]+"').Replace($text, "`$1`"$Version`"", 2)
[IO.File]::WriteAllText($lock, $text, $utf8)

# Cargo.lock подтянет новые версии своих пакетов сам, без сети.
Push-Location $root
try { cargo update --workspace --offline } finally { Pop-Location }
Write-Host "Версия $Version. Дальше: коммит, затем git tag v$Version и git push origin v$Version"
