# Окружение сборки: добавляет Rust, clang (llvm-mingw) и Git из папки work\ в PATH
# и задаёт переменные для Cargo. Подключается скриптами сборки командой:
#   . (Join-Path $workspace 'work/rust-env.ps1')
# Все пути строятся от корня репозитория, личных путей здесь нет.
$taskRoot = Split-Path -Parent $PSScriptRoot
$rustBin = Join-Path $taskRoot 'work/toolchain/bin'
$llvmBin = Join-Path $taskRoot 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/bin'
$gitBin = Join-Path $taskRoot 'work/mingit/cmd'
$env:PATH = $llvmBin + ';' + $rustBin + ';' + $gitBin + ';' + $env:PATH
$env:CARGO_HOME = Join-Path $taskRoot 'work/cargo-home'
$env:RUSTC = Join-Path $rustBin 'rustc.exe'
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_BUILD_JOBS = '4'
$env:RUSTFLAGS = '-C link-self-contained=yes'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $llvmBin 'x86_64-w64-mingw32-clang.exe'
$env:CC = Join-Path $llvmBin 'x86_64-w64-mingw32-clang.exe'
$env:CXX = Join-Path $llvmBin 'x86_64-w64-mingw32-clang++.exe'
$env:AR = Join-Path $llvmBin 'llvm-ar.exe'
