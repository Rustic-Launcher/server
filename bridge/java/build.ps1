# SPDX-License-Identifier: GPL-3.0-only
#
# Windows PowerShell twin of build.sh: fetch the Paper API and its libraries (deps.txt, SHA-256
# checked) into build\libs, compile with javac --release 25, build the test plugins, run the unit
# tests, package build\rustic-bridge.jar.
#
#   powershell -ExecutionPolicy Bypass -File bridge\java\build.ps1 [-Offline] [-SkipTests]
param([switch]$Offline, [switch]$SkipTests)
$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $here '..\..')).Path
$build = Join-Path $here 'build'
if ($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin\javac.exe'))) { $jbin = Join-Path $env:JAVA_HOME 'bin' }
elseif (Test-Path (Join-Path $repo 'tools\jdk25\bin\javac.exe')) { $jbin = Join-Path $repo 'tools\jdk25\bin' }
else { $jbin = Split-Path (Get-Command javac).Source }
$javac = Join-Path $jbin 'javac.exe'; $jar = Join-Path $jbin 'jar.exe'; $java = Join-Path $jbin 'java.exe'

$repoUrl = 'https://repo.papermc.io/repository/maven-public'
New-Item -ItemType Directory -Force (Join-Path $build 'libs'), (Join-Path $build 'test-libs') | Out-Null
foreach ($line in Get-Content (Join-Path $here 'deps.txt')) {
    if ($line -match '^\s*(#|$)') { continue }
    $scope, $coord, $want = $line -split '\s+'
    $g, $a, $v = $coord -split ':'
    $dir = if ($scope -eq 'runtime') { Join-Path $build 'libs' } else { Join-Path $build 'test-libs' }
    $file = Join-Path $dir "$a-$v.jar"
    if ((Test-Path $file) -and ((Get-FileHash $file -Algorithm SHA256).Hash.ToLower() -eq $want)) { continue }
    if ($Offline) { throw "$file missing (offline)" }
    Write-Host "fetch $coord"
    $url = "$repoUrl/$($g.Replace('.', '/'))/$a/$v/$a-$v.jar"
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile "$file.part"
    $got = (Get-FileHash "$file.part" -Algorithm SHA256).Hash.ToLower()
    if ($got -ne $want) { Remove-Item "$file.part"; throw "${coord}: SHA-256 $got, expected $want" }
    Move-Item -Force "$file.part" $file
}

$cpLibs = (Join-Path $build 'libs') + '\*'
$flags = @('--release', '25', '-encoding', 'UTF-8', '-Xlint:all,-processing,-serial,-classfile,-deprecation,-removal')
function Compile($src, $out, $cp) {
    if (Test-Path $out) { Remove-Item -Recurse -Force $out }
    New-Item -ItemType Directory -Force $out | Out-Null
    $list = Join-Path $build 'sources.txt'
    Get-ChildItem -Recurse -Filter *.java $src | ForEach-Object { $_.FullName } | Set-Content -Encoding ascii $list
    & $javac @flags -cp $cp -d $out "@$list"
    if ($LASTEXITCODE -ne 0) { throw "javac failed for $src" }
}

Write-Host 'javac bridge'
$classes = Join-Path $build 'classes'
Compile (Join-Path $here 'src\main\java') $classes $cpLibs
Copy-Item -Recurse -Force (Join-Path $here 'src\main\resources\*') $classes
Copy-Item -Force (Join-Path $here 'COPYING') (Join-Path $classes 'COPYING.rustic-bridge')
"Main-Class: rustic.bridge.Main`nImplementation-Title: Rustic Bridge`nImplementation-Version: 0.1.0`n" |
    Set-Content -Encoding ascii -NoNewline (Join-Path $build 'manifest.txt')
& $jar --create --file (Join-Path $build 'rustic-bridge.jar') --manifest (Join-Path $build 'manifest.txt') -C $classes .
if ($LASTEXITCODE -ne 0) { throw 'jar failed' }

New-Item -ItemType Directory -Force (Join-Path $build 'plugins') | Out-Null
foreach ($dir in Get-ChildItem -Directory (Join-Path $here 'test-plugins')) {
    $out = Join-Path $build "plugin-classes\$($dir.Name)"
    Write-Host "javac plugin $($dir.Name)"
    Compile (Join-Path $dir.FullName 'src') $out $cpLibs
    Copy-Item -Recurse -Force (Join-Path $dir.FullName 'resources\*') $out
    $shade = Join-Path $dir.FullName 'shade.txt'
    if (Test-Path $shade) {
        foreach ($j in Get-Content $shade) {
            if (-not $j) { continue }
            Push-Location $out; & $jar --extract --file (Join-Path $build "test-libs\$j"); Pop-Location
        }
        Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $out 'META-INF\MANIFEST.MF')
    }
    $name = (Select-String -Path (Join-Path $dir.FullName 'resources\plugin.yml') -Pattern '^name:\s*(.+)$').Matches[0].Groups[1].Value.Trim()
    & $jar --create --file (Join-Path $build "plugins\$name.jar") -C $out .
    if ($LASTEXITCODE -ne 0) { throw "jar failed for $name" }
}

if (-not $SkipTests) {
    Write-Host 'javac tests'
    $tests = Join-Path $build 'test-classes'
    Compile (Join-Path $here 'src\test\java') $tests "$classes;$cpLibs"
    Write-Host 'run tests'
    & $java -cp "$tests;$classes;$cpLibs" "-Drustic.test.plugins=$(Join-Path $build 'plugins')" rustic.bridge.TestMain
    if ($LASTEXITCODE -ne 0) { throw 'tests failed' }
}
Write-Host "built $(Join-Path $build 'rustic-bridge.jar')"
