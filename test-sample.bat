@echo off
setlocal

rem Run one sample file at given options
rem reads input from samples\%1
rem creates output in out\%1
rem Provide "x86" or "x64" as first param to set target.

if not exist out mkdir out

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
    set target=i686-pc-windows-msvc
    shift
) else if "x%1"=="xx64" (
    set arch=x64
    set target=x86_64-pc-windows-msvc
    shift
) else (
    set arch=x64
    set target=x86_64-pc-windows-msvc
)

call %VsDevCmd% -arch=%arch% -host_arch=%arch%

set x=%1
shift
cd samples
cargo run --release --target=%target%  --features=cli -- %1 %2 %3 %4 %5 %6 %7 %8 %9 %x% ../out/%x%
cd ..
