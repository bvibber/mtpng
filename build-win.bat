@echo off
setlocal

rem C API build script for Windows.
rem Must be run from a Visual Studio Dev Tools console window

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
call VsDevCmd -arch=%arch% -host_arch=%hostarch%

%CC% %CFLAGS% /Fe%EXE% %SOURCES%  %LDFLAGS% /link
if %errorlevel% neq 0 exit /b %errorlevel%

if not exist out mkdir out
%CDYLIB_PATHVAR% .\%EXE%
