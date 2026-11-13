# Быстрая установка: свяжите нашу сборку с вашей установкой Homestead и запустите игру.
#
# Что нужно: (1) наша сборка (папка или .zip), (2) основная сборка Homestead из CurseForge
# (папка экземпляра). Скрипт сам найдёт ресурсы Horizon, проверит файлы, запишет
# local-settings.json и предложит запустить игру. Ничего чужого он не скачивает и не копирует
# в нашу сборку, кроме приватной копии Create JAR, которую делает сам Start-Homestead.
#
# Запуск: двойной щелчок по Install-Homestead.cmd или
#   powershell -ExecutionPolicy Bypass -File Install-Homestead.ps1
# Без вопросов:
#   Install-Homestead.ps1 -OurBuild "D:\Games\homestead-horizon-stage19" -Homestead "C:\...\Instances\Homestead" -Launch
[CmdletBinding()]
param(
    [string]$OurBuild,        # папка или .zip нашей сборки
    [string]$Homestead,       # папка экземпляра Homestead (CurseForge)
    [string]$Resources,       # папка horizon-client\26.3 (с facts.json), если не найдётся сама
    [string]$InstallTo,       # куда распаковать, если OurBuild это .zip
    [switch]$Launch,          # сразу запустить после установки
    [switch]$CheckOnly        # только проверить, ничего не писать и не запускать
)

$ErrorActionPreference = 'Stop'
$ExpectedCreateJar = 'create-fabric-6.0.8.1+build.1744-mc1.20.1.jar'
$ExpectedCreateHash = 'a613603fb08d967c5a4138d1adb7374c4a347b91e489fe21579a4cff4f40105a'

function Say($text, $color) {
    if ($color) { Write-Host $text -ForegroundColor $color } else { Write-Host $text }
}
function Fail($text) { Say ('ОШИБКА: ' + $text) 'Red'; exit 1 }
function Ask($prompt, $default) {
    $suffix = ''
    if ($default) { $suffix = ' [' + $default + ']' }
    $answer = Read-Host ($prompt + $suffix)
    if ([string]::IsNullOrWhiteSpace($answer)) { return $default }
    return $answer.Trim().Trim('"')
}
function IsBuildFolder($path) {
    return ($path -and (Test-Path -LiteralPath (Join-Path $path 'client\horizon-client.exe')) -and
        (Test-Path -LiteralPath (Join-Path $path 'Start-Homestead.ps1')))
}

Say ''
Say '=== Установка Homestead Horizon ===' 'Cyan'
Say ''

# --- 0. Базовые проверки -------------------------------------------------------------
if ($PSVersionTable.PSVersion.Major -lt 5) { Fail 'Нужен PowerShell 5 или новее.' }
if ([Environment]::OSVersion.Platform -ne 'Win32NT') { Fail 'Сборка работает только в Windows.' }

# --- 1. Наша сборка ------------------------------------------------------------------
if (-not $OurBuild -and (IsBuildFolder $PSScriptRoot)) { $OurBuild = $PSScriptRoot }
if (-not $OurBuild) {
    Say 'Шаг 1 из 3. Наша сборка.' 'Yellow'
    Say 'Укажите папку, куда вы распаковали нашу сборку (внутри лежат client и Start-Homestead.ps1),'
    Say 'или путь к скачанному .zip.'
    $OurBuild = Ask 'Путь к нашей сборке' ''
}
if (-not $OurBuild) { Fail 'Путь к нашей сборке не указан.' }
$OurBuild = $OurBuild.Trim('"')
if (-not (Test-Path -LiteralPath $OurBuild)) { Fail ('Не найдено: ' + $OurBuild) }

$buildRoot = $null
if ((Get-Item -LiteralPath $OurBuild).PSIsContainer) {
    $buildRoot = (Resolve-Path -LiteralPath $OurBuild).Path
    # Если выбрали папку выше (внутри одна вложенная папка со сборкой), найдём её.
    if (-not (IsBuildFolder $buildRoot)) {
        $inner = Get-ChildItem -LiteralPath $buildRoot -Directory -ErrorAction SilentlyContinue |
            Where-Object { IsBuildFolder $_.FullName } | Select-Object -First 1
        if ($inner) { $buildRoot = $inner.FullName }
    }
} elseif ($OurBuild -match '\.zip$') {
    if (-not $InstallTo) {
        $default = Join-Path (Split-Path -Parent (Resolve-Path -LiteralPath $OurBuild).Path) ([IO.Path]::GetFileNameWithoutExtension($OurBuild))
        $InstallTo = Ask 'Куда распаковать (потом папку нельзя переносить и переименовывать)' $default
    }
    if ($CheckOnly) {
        Say ('Проверка: архив был бы распакован в ' + $InstallTo) 'DarkGray'
        $buildRoot = $InstallTo
    } else {
        if ((Test-Path -LiteralPath $InstallTo) -and (Get-ChildItem -LiteralPath $InstallTo -Force | Select-Object -First 1)) {
            Fail ('Папка не пуста: ' + $InstallTo + '. Укажите пустую или несуществующую.')
        }
        Say ('Распаковываю в ' + $InstallTo + ' (около минуты)...')
        New-Item -ItemType Directory -Force -Path $InstallTo | Out-Null
        Expand-Archive -LiteralPath $OurBuild -DestinationPath $InstallTo
        $buildRoot = $InstallTo
        if (-not (IsBuildFolder $buildRoot)) {
            $inner = Get-ChildItem -LiteralPath $buildRoot -Directory | Where-Object { IsBuildFolder $_.FullName } | Select-Object -First 1
            if ($inner) { $buildRoot = $inner.FullName }
        }
    }
} else {
    Fail 'Нужна папка со сборкой или файл .zip.'
}
if (-not $CheckOnly -and -not (IsBuildFolder $buildRoot)) {
    if (Test-Path -LiteralPath (Join-Path $buildRoot 'work\homestead-horizon\stage4')) {
        Say ''
        Say 'Это не готовая сборка, а ИСХОДНЫЙ КОД проекта (скачанный с GitHub).' 'Red'
        Say 'В нём нет игры: клиента и модов в репозиторий не кладут, потому что они чужие.' 'Red'
        Say ''
        Say 'Что делать:'
        Say '  1. Если автор передал вам готовую сборку (папку homestead-horizon-stageNN или .zip),'
        Say '     запустите установку заново и укажите путь к ней, а не к репозиторию.'
        Say '  2. Если готовой сборки нет, её можно собрать из исходников самому:'
        Say '     см. INSTALL.md, путь Б (нужны Rust, исходники Horizon и около часа).'
        exit 1
    }
    Fail ('В ' + $buildRoot + ' не нашёл client\horizon-client.exe и Start-Homestead.ps1. Это не папка нашей сборки или архив распакован не до конца.')
}
Say ('Наша сборка: ' + $buildRoot) 'Green'
if ($buildRoot.Length -gt 100) { Say 'Предупреждение: путь очень длинный, у Windows бывают проблемы. Лучше короче.' 'Yellow' }

# --- 2. Основная сборка Homestead ----------------------------------------------------
if (-not $Homestead) {
    $candidates = @()
    foreach ($root in @(
        (Join-Path $env:USERPROFILE 'curseforge\minecraft\Instances'),
        (Join-Path $env:USERPROFILE 'Documents\curseforge\minecraft\Instances'),
        (Join-Path $env:USERPROFILE 'Documents\CurseForge\minecraft\Instances'))) {
        if (Test-Path -LiteralPath $root) {
            $candidates += @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -like '*Homestead*' } | ForEach-Object { $_.FullName })
        }
    }
    Say ''
    Say 'Шаг 2 из 3. Основная сборка Homestead (CurseForge).' 'Yellow'
    $default = ''
    if ($candidates.Count -gt 0) {
        $default = $candidates[0]
        Say ('Нашёл: ' + $default)
    } else {
        Say 'Укажите папку экземпляра Homestead: внутри неё лежит папка mods.'
    }
    $Homestead = Ask 'Путь к Homestead' $default
}
if (-not $Homestead) { Fail 'Путь к Homestead не указан.' }
$Homestead = $Homestead.Trim('"')
$createJar = Join-Path $Homestead ('mods\' + $ExpectedCreateJar)
if (-not (Test-Path -LiteralPath $createJar)) {
    Fail ('В ' + $Homestead + '\mods нет файла ' + $ExpectedCreateJar + '. Нужна сборка Homestead - A Cozy Survival Experience целиком (скачайте её в CurseForge и дождитесь установки модов).')
}
Say 'Проверяю файл Create (хэш)...'
$hash = (Get-FileHash -LiteralPath $createJar -Algorithm SHA256).Hash.ToLowerInvariant()
if ($hash -ne $ExpectedCreateHash) {
    Fail 'Файл Create в вашем Homestead отличается от проверенной версии. Нужна та же версия сборки, что у автора (Create 6.0.8.1).'
}
Say ('Homestead: ' + $Homestead) 'Green'

# --- 3. Ресурсы Horizon --------------------------------------------------------------
if (-not $Resources) {
    $found = @()
    $launcher = Join-Path $env:APPDATA 'horizon-launcher\versions'
    if (Test-Path -LiteralPath $launcher) {
        $found = @(Get-ChildItem -LiteralPath $launcher -Directory -ErrorAction SilentlyContinue | ForEach-Object {
            Join-Path $_.FullName '.local\horizon-client\26.3'
        } | Where-Object { Test-Path -LiteralPath (Join-Path $_ 'facts.json') })
    }
    Say ''
    Say 'Шаг 3 из 3. Ресурсы Horizon для Minecraft 26.3.' 'Yellow'
    $default = ''
    if ($found.Count -gt 0) {
        $default = ($found | Sort-Object { (Get-Item -LiteralPath $_).LastWriteTime } -Descending | Select-Object -First 1)
        Say ('Нашёл: ' + $default)
    } else {
        Say 'Не нашёл автоматически. Откройте Horizon Launcher, запустите любую версию 26.3, затем укажите'
        Say 'папку horizon-client\26.3 (в ней лежит facts.json).'
    }
    $Resources = Ask 'Путь к ресурсам Horizon' $default
}
if (-not $Resources) { Fail 'Путь к ресурсам Horizon не указан.' }
$Resources = $Resources.Trim('"')
if (-not (Test-Path -LiteralPath (Join-Path $Resources 'facts.json'))) {
    Fail ('В ' + $Resources + ' нет facts.json. Это не папка ресурсов Horizon 26.3.')
}
Say ('Ресурсы Horizon: ' + $Resources) 'Green'

# --- Свободное место и запущенная игра -----------------------------------------------
$drive = (Get-Item -LiteralPath $buildRoot -ErrorAction SilentlyContinue)
if ($drive) {
    $letter = $drive.PSDrive.Name
    $free = (Get-PSDrive -Name $letter -ErrorAction SilentlyContinue).Free
    if ($free -and $free -lt 3GB) { Say ('Предупреждение: на диске ' + $letter + ': меньше 3 ГБ свободного места, игре нужно около 3 ГБ.') 'Yellow' }
}
$running = @(Get-Process -Name 'horizon-client' -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) { Say 'Предупреждение: уже запущена игра Horizon. Закройте её перед запуском, две копии портят кэш.' 'Yellow' }

# --- Запись настроек -----------------------------------------------------------------
if ($CheckOnly) {
    Say ''
    Say 'Проверка завершена: всё найдено, ничего не записано.' 'Cyan'
    exit 0
}
$settings = [ordered]@{ resources = $Resources; sourceInstance = $Homestead }
$settingsFile = Join-Path $buildRoot 'local-settings.json'
[IO.File]::WriteAllText($settingsFile, ($settings | ConvertTo-Json), (New-Object Text.UTF8Encoding($false)))
Say ''
Say ('Настройки записаны: ' + $settingsFile) 'Green'
Say ''
Say 'Готово. Первый запуск долгий: 3-5 минут игра строит кэш модов. Это не зависание:' 'Cyan'
Say 'не закрывайте окно и не запускайте вторую копию.' 'Cyan'

if (-not $Launch) {
    $answer = Ask 'Запустить игру сейчас? (да/нет)' 'да'
    $Launch = ($answer -match '^(д|y)')
}
if ($Launch) {
    Say 'Запускаю...'
    & (Join-Path $buildRoot 'Play-Create.cmd')
} else {
    Say ('Запуск позже: двойной щелчок по ' + (Join-Path $buildRoot 'Play-Create.cmd'))
}
