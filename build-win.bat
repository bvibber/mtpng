@echo off
setlocal

rem C API build script for Windows. Having trouble making it work
rem for x86 on an x64 machine at the moment.

if "x%VSCMD_ARG_TGT_ARCH%"=="x" (
    echo Must run from inside a Visual Studio tools command prompt.
    exit /b 1
)

if "%VSCMD_ARG_TGT_ARCH%"=="x86" (
    set TARGET=i686-pc-windows-msvc
) else if "%VSCMD_ARG_TGT_ARCH%"=="x64" (
    set TARGET=x86_64-pc-windows-msvc
) else (
    echo "Unrecognized arch %VSCMD_ARG_TGT_ARCH% not yet supported."
    exit /b 1
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

%CARGO% build --target=%TARGET% --release --features capi
if %errorlevel% neq 0 exit /b %errorlevel%

if not exist build mkdir build
copy %RUSTLIBDIR%\mtpng.dll.lib build\mtpng.lib
copy %RUSTLIBDIR%\mtpng.dll build\mtpng.dll

%CC% %CFLAGS% /Fe%EXE% %SOURCES%  %LDFLAGS% /link
if %errorlevel% neq 0 exit /b %errorlevel%

if not exist out mkdir out
%CDYLIB_PATHVAR% .\%EXE%
