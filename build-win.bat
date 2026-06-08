@echo off
setlocal

rem C API build script for Windows.
rem Must be run from a Visual Studio Dev Tools console window

goto :Main



:BuildArch

set CARGO=cargo
set PROFILE=release
set RUSTLIBDIR=target\%target%\%PROFILE%

set SOURCES=c\sample.c
set HEADERS=c\mtpng.h
set EXE=build\%target%\sample.exe
if not exist build mkdir build
if not exist build\%target% mkdir build\%target%
if not exist build\%target%\out mkdir build\%target%\out

rustup target add %target%

%CARGO% build --target=%target% --release --features capi
if %errorlevel% neq 0 exit /b %errorlevel%

copy %RUSTLIBDIR%\mtpng.dll.lib build\%target%\mtpng.lib
copy %RUSTLIBDIR%\mtpng.dll build\%target%\mtpng.dll

rem Now set up and build our C app!
call VsDevCmd -arch=%arch% -host_arch=%hostarch%

cl /Fe%EXE% %SOURCES% build\%target%\mtpng.lib /link
if %errorlevel% neq 0 exit /b %errorlevel%

exit /b



:Main

set arch=x86
set hostarch=x64
set target=i686-pc-windows-msvc
call :BuildArch

set arch=x64
set hostarch=x64
set target=x86_64-pc-windows-msvc
call :BuildArch

set arch=arm64
set hostarch=x64
set target=aarch64-pc-windows-msvc
call :BuildArch
