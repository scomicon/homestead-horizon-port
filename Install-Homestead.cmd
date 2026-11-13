@echo off
setlocal
chcp 65001 >nul
if not exist "%~dp0Install-Homestead.ps1" (
  echo.
  echo ОШИБКА: рядом с этим файлом нет Install-Homestead.ps1.
  echo Вероятно, вы запустили его прямо из архива .zip: Windows вытаскивает из архива
  echo только один файл, а остальные не распаковывает.
  echo.
  echo Что делать: закройте это окно, в проводнике нажмите правой кнопкой по архиву,
  echo выберите "Извлечь все...", распакуйте в обычную папку, например D:\Games,
  echo и запустите Install-Homestead.cmd уже из неё.
  echo.
  pause
  exit /b 1
)
set "PSModulePath=%SystemRoot%\System32\WindowsPowerShell\v1.0\Modules;%ProgramFiles%\WindowsPowerShell\Modules;%USERPROFILE%\Documents\WindowsPowerShell\Modules"
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install-Homestead.ps1" %*
echo.
pause
endlocal
