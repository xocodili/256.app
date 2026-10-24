# Run cargo with the same environment as "x64 Native Tools Command Prompt for VS 2022".
# Required on Windows when `cargo` fails with: linker `link.exe` not found, or LNK1181 kernel32.lib.

param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

$vcvars = @(
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $vcvars) {
    Write-Error "vcvars64.bat not found. Install 'Desktop development with C++' or MSVC v143 build tools + Windows SDK."
    exit 1
}

$tmp = New-TemporaryFile
cmd /c "`"$vcvars`" && set > `"$tmp`""
if ($LASTEXITCODE -ne 0) {
    Write-Error "vcvars64.bat failed (exit $LASTEXITCODE): $vcvars"
    exit $LASTEXITCODE
}
Get-Content $tmp | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') {
        Set-Item -Path "env:$($matches[1])" -Value $matches[2]
    }
}
Remove-Item $tmp -ErrorAction SilentlyContinue

$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) { $cargo = "cargo" }
& $cargo @CargoArgs
exit $LASTEXITCODE
