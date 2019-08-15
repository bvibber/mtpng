@echo off
setlocal

rem C API build script for Windows.


rem no really, this is how you look this up
rem https://docs.microsoft.com/en-us/dotnet/csharp/language-reference/compiler-options/how-to-set-environment-variables-for-the-visual-studio-command-line

set prog=C:\Program Files
set prog86=C:\Program Files (x86)
set vs=Microsoft Visual Studio\2019
set common=Common7\Tools\VsDevCmd.bat
if exist "%prog%\%vs%\Community\%common%" (
    set VsDevCmd="%prog%\%vs%\Community\%common%"
) else if exist "%prog%\%vs%\Professional\%common%" (
    set VsDevCmd="%prog%\%vs%\Professional\%common%"
) else if exist "%prog%\%vs%\Enterprise\%common%" (
    set VsDevCmd="%prog%\%vs%\Enterprise\%common%"
) else if exist "%prog%\%vs%\BuildTools\%common%" (
    set VsDevCmd="%prog%\%vs%\BuildTools\%common%"
) else if exist "%prog86%\%vs%\Community\%common%" (
    set VsDevCmd="%prog86%\%vs%\Community\%common%"
) else if exist "%prog86%\%vs%\Professional\%common%" (
    set VsDevCmd="%prog86%\%vs%\Professional\%common%"
) else if exist "%prog86%\%vs%\Enterprise\%common%" (
    set VsDevCmd="%prog86%\%vs%\Enterprise\%common%"
) else if exist "%prog86%\%vs%\BuildTools\%common%" (
    set VsDevCmd="%prog86%\%vs%\BuildTools\%common%"
) else (
    echo "Could not find Visual Studio dev tools."
)

if "x%1"=="xx86" (
    set arch=x86
    set hostarch=x86
    set target=i686-pc-windows-msvc
    shift
) else if "x%1"=="xx64" (
    set arch=x64
    set hostarch=x64
    set target=x86_64-pc-windows-msvc
    shift
) else if "x%1"=="xarm64" (
    set arch=arm64
    set hostarch=x86
    set target=aarch64-pc-windows-msvc
    shift
) else (
    set arch=x64
    set hostarch=x64
    set target=x86_64-pc-windows-msvc
)

set CARGO=cargo
set PROFILE=release
set RUSTLIBDIR=target\%TARGET%\%PROFILE%

rem This is all hacky and not gonna help with cross-compiling. :D
set CC=cl
set CFLAGS=
set LDFLAGS=build\mtpng.lib

set SOURCES=c\sample.c
set HEADERS=c\mtpng.h
set EXE=build\sample.exe

%CARGO% build --target=%target% --release --features capi
if %errorlevel% neq 0 exit /b %errorlevel%

if not exist build mkdir build
copy %RUSTLIBDIR%\mtpng.dll.lib build\mtpng.lib
copy %RUSTLIBDIR%\mtpng.dll build\mtpng.dll


rem Now set up and build our C app!
call %VsDevCmd% -arch=%arch% -host_arch=%hostarch%

%CC% %CFLAGS% /Fe%EXE% %SOURCES%  %LDFLAGS% /link
if %errorlevel% neq 0 exit /b %errorlevel%

if not exist out mkdir out
%CDYLIB_PATHVAR% .\%EXE%
