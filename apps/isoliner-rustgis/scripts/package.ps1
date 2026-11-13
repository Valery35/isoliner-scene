# Isoliner Scene - сцена Isoliner3D без QGIS.
# Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
# SPDX-License-Identifier: Apache-2.0
#
# Пакет .rgplugin расширения Isoliner3D для RustGIS, по образцу RustPlanetX
# (C:\Dev\rustplanetx\scripts\package.ps1). В пакете plugin.json и одна
# программа bin/isoliner-rustgis.exe: она отвечает RustGIS (rustgis-plugin)
# и сама же открывает окно сцены (scene файл.i3dscene).
#
#   cargo build --release -p isoliner-rustgis
#   powershell -File apps/isoliner-rustgis/scripts/package.ps1
#
# В dist/ (корень хранилища) появляются:
#   isoliner3d/                  plugin.json и bin/ - папка для
#                                «Add Plugin Folder…» и rustgis-cli
#   isoliner3d-<версия>.rgplugin пакет для установки из файла

param(
    [string]$Windows = ''
)

$ErrorActionPreference = 'Stop'
$app = Split-Path -Parent $PSScriptRoot
$root = Split-Path -Parent (Split-Path -Parent $app)
if (-not $Windows) { $Windows = Join-Path $root 'target\release\isoliner-rustgis.exe' }
if (-not (Test-Path $Windows)) { throw "no program at ${Windows} - cargo build --release -p isoliner-rustgis" }
$dist = Join-Path $root 'dist'
$manifestText = [IO.File]::ReadAllText((Join-Path $app 'plugin.json'), [Text.Encoding]::UTF8)
$manifest = $manifestText | ConvertFrom-Json
$id = $manifest.id
$version = $manifest.version
if (-not $manifestText.Contains('"windows": ["bin/isoliner-rustgis.exe"')) { throw 'plugin.json run has no bin/isoliner-rustgis.exe' }

$files = @(
    @{ path = 'plugin.json'; bytes = (New-Object Text.UTF8Encoding($false)).GetBytes($manifestText) },
    @{ path = 'bin/isoliner-rustgis.exe'; bytes = [IO.File]::ReadAllBytes((Resolve-Path $Windows).Path) }
)

New-Item -ItemType Directory -Force $dist | Out-Null
$folder = Join-Path $dist $id
if (Test-Path $folder) { Remove-Item -Recurse -Force $folder -Confirm:$false }
foreach ($f in $files) {
    $target = $folder
    foreach ($part in $f.path.Split('/')) { $target = Join-Path $target $part }
    New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
    [IO.File]::WriteAllBytes($target, $f.bytes)
}

# Пакет - ZIP с файлами в корне, время записей одно: одинаковые файлы
# дают одинаковый пакет и SHA-256.
Add-Type -AssemblyName System.IO.Compression
$path = Join-Path $dist "$id-$version.rgplugin"
if (Test-Path $path) { Remove-Item $path -Confirm:$false }
$stream = [IO.File]::Open($path, [IO.FileMode]::CreateNew)
try {
    $zip = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($f in $files) {
            $entry = $zip.CreateEntry($f.path, [IO.Compression.CompressionLevel]::Optimal)
            $entry.LastWriteTime = [DateTimeOffset]::new(2024, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
            $out = $entry.Open()
            try { $out.Write($f.bytes, 0, $f.bytes.Length) } finally { $out.Dispose() }
        }
    } finally { $zip.Dispose() }
} finally { $stream.Dispose() }
$size = (Get-Item $path).Length
# RustGIS не берёт пакет больше 64 МиБ.
if ($size -gt 64MB) { throw ("{0} is {1:N0} bytes, over 64 MiB" -f (Split-Path -Leaf $path), $size) }
$sha = (Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Output ("dist/{0}  {1:N0} bytes  sha256 {2}" -f (Split-Path -Leaf $path), $size, $sha)
