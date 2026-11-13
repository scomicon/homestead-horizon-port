#requires -Version 7.0
# Создаёт три файла с путями, которые читают скрипты сборки, и проверяет, что на месте
# инструменты и исходники. Запускайте из любого места:
#   pwsh work\homestead-horizon\Setup-Paths.ps1
# Файлы создаются в work\homestead-horizon\ и игнорируются git (в них абсолютные пути вашей машины).
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$base = $PSScriptRoot
$root = [IO.Path]::GetFullPath((Join-Path $base '../..'))

$createRev = '4ae7340c06da5e77828e12494509fe9a70b8eaa4'
$engineRev = '4e5296d1800b3f89f2ae48dfd99d431801df4d46'
$mixinsRev = 'e8cd0aa38cffef49cb2e088ef716baa107148c02'

$targets = [ordered]@{
    'create-source-path.txt'  = Join-Path $base "create-source/create-horizon-$createRev"
    'engine-source-path.txt'  = Join-Path $base "engine/horizon-$engineRev"
    'mixins-source-path.txt'  = Join-Path $base "mixins-tool/horizon-mixins-$mixinsRev"
}

$missing = @()
foreach ($name in $targets.Keys) {
    $dir = $targets[$name]
    if (Test-Path -LiteralPath $dir -PathType Container) {
        Set-Content -LiteralPath (Join-Path $base $name) -Value $dir -Encoding utf8NoBOM
        Write-Output ("OK      {0} -> {1}" -f $name, $dir)
    } else {
        Write-Output ("НЕТ     {0}: папка не найдена: {1}" -f $name, $dir)
        $missing += $dir
    }
}

# Проверка инструментов (только сообщение, ничего не создаётся).
$tools = [ordered]@{
    'Rust (cargo)'      = Join-Path $root 'work/toolchain/bin/cargo.exe'
    'rustfmt'           = Join-Path $root 'work/toolchain/bin/rustfmt.exe'
    'clang (llvm-mingw)'= Join-Path $root 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/bin/x86_64-w64-mingw32-clang.exe'
    'Python (llvm-mingw)'= Join-Path $root 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
    'Git (mingit)'      = Join-Path $root 'work/mingit/cmd/git.exe'
    'horizon-mixins'    = Join-Path $targets['mixins-source-path.txt'] 'target/debug/horizon-mixins.exe'
    'local-settings.json' = Join-Path $root 'local-settings.json'
}
$bad = 0
foreach ($name in $tools.Keys) {
    if (Test-Path -LiteralPath $tools[$name]) { Write-Output ("OK      {0}" -f $name) }
    else { Write-Output ("НЕТ     {0}: {1}" -f $name, $tools[$name]); $bad++ }
}

if ($missing.Count -gt 0 -or $bad -gt 0) {
    Write-Output ''
    Write-Output 'Не всё готово. Что делать для каждой строки «НЕТ»: INSTALL.md, разделы Б.1-Б.3.'
    exit 1
}
Write-Output ''
Write-Output 'Всё на месте. Дальше: pwsh work\homestead-horizon\stage4\build.ps1'
