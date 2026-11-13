#requires -Version 7.0
# Собирает .hmod из ВАШИХ собственных JAR в локальную игнорируемую папку.
# Ничего чужого скрипт не скачивает и не публикует. Результат нельзя выкладывать.
#
# Нужен local-settings.json в корне репозитория (образец: local-settings.example.json):
#   resources      путь к подготовленным ресурсам Horizon 26.3
#   sourceInstance папка вашей установки Homestead (для JAR Create)
#
# Сейчас скрипт собирает мод Farmer's Delight из вашего JAR через Build-FarmersDelight.ps1.
# Остальные «данные»-моды (Build-DataMods.ps1) требуют каталога work/homestead-horizon/pack/mods/<мод>/carry,
# который создаёт `horizon-port-pack carry` из ваших JAR. Этот шаг пока ручной, см. README.md.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$settingsFile = Join-Path $workspace 'local-settings.json'
if (-not (Test-Path -LiteralPath $settingsFile)) {
    throw "Нет local-settings.json. Скопируйте local-settings.example.json и впишите свои пути."
}
$settings = Get-Content -LiteralPath $settingsFile -Raw | ConvertFrom-Json
foreach ($key in 'resources', 'sourceInstance') {
    $value = $settings.$key
    if (-not $value -or $value -like '<*') { throw "В local-settings.json не заполнено поле '$key'." }
    if (-not (Test-Path -LiteralPath $value)) { throw "Путь из '$key' не существует: $value" }
}
$create = Get-ChildItem (Join-Path $settings.sourceInstance 'mods') -Filter 'create-fabric-*.jar' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $create) { Write-Warning 'JAR Create не найден в вашей установке: оригинальные текстуры Create будут недоступны.' }
$pathFile = Join-Path $workspace 'work/homestead-horizon/create-source-path.txt'
if (-not (Test-Path -LiteralPath $pathFile)) {
    $src = Get-ChildItem (Join-Path $workspace 'work/homestead-horizon/create-source') -Directory | Select-Object -First 1
    if (-not $src) { throw 'Не найден work/homestead-horizon/create-source/*.' }
    [IO.File]::WriteAllText($pathFile, $src.FullName)
}
Write-Output 'Сборка Farmer''s Delight из вашего JAR...'
& (Join-Path $PSScriptRoot 'Build-FarmersDelight.ps1')
Write-Output 'Готово. Файлы .hmod лежат в stage4/extra-mods (игнорируется git). Не публикуйте их.'
