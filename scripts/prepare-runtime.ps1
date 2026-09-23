$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$downloadDir = Join-Path $projectRoot '.build/downloads'
$runtimeDir = Join-Path $projectRoot 'src-tauri/runtime'
$androidLibDir = Join-Path $projectRoot 'android/app/libs'
New-Item -ItemType Directory -Force -Path $downloadDir,$runtimeDir,$androidLibDir | Out-Null
$packages = @(
    @{Name='xray-26.7.28';Url='https://github.com/XTLS/Xray-core/releases/download/v26.7.28/Xray-windows-64.zip';Hash='c7172078fca4711bcd92a4774dcd1822544579c58816197575c47533317fd8d1'},
    @{Name='wintun';Url='https://www.wintun.net/builds/wintun-0.14.1.zip';Hash='07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51'},
    @{Name='libxray';Url='https://github.com/XTLS/libXray/releases/download/v26.7.28/libxray-android.zip';Hash='28b7dc9d6cc8455fcca5cbd56e387003a7bfb558128651a64899dc3a8ccff666'}
)
foreach($package in $packages) {
    $archive = Join-Path $downloadDir ($package.Name+'.zip')
    if(!(Test-Path -LiteralPath $archive)) {Invoke-WebRequest -Uri $package.Url -OutFile $archive -TimeoutSec 180}
    if((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $package.Hash){throw "Checksum mismatch: $($package.Name)"}
    $extracted = Join-Path $downloadDir $package.Name
    if(!(Test-Path -LiteralPath $extracted)){Expand-Archive -LiteralPath $archive -DestinationPath $extracted}
}
Copy-Item -LiteralPath (Join-Path $downloadDir 'xray-26.7.28/xray.exe') -Destination $runtimeDir -Force
Copy-Item -LiteralPath (Join-Path $downloadDir 'xray-26.7.28/LICENSE') -Destination (Join-Path $runtimeDir 'LICENSE-Xray.txt') -Force
Copy-Item -LiteralPath (Join-Path $downloadDir 'wintun/wintun/bin/amd64/wintun.dll') -Destination $runtimeDir -Force
Copy-Item -LiteralPath (Join-Path $downloadDir 'wintun/wintun/LICENSE.txt') -Destination (Join-Path $runtimeDir 'LICENSE-Wintun.txt') -Force
$aar = @(Get-ChildItem -LiteralPath (Join-Path $downloadDir 'libxray') -Filter 'libXray.aar' -Recurse)
if($aar.Count -ne 1){throw 'Expected exactly one libXray.aar'}
if((Get-FileHash -LiteralPath $aar[0].FullName -Algorithm SHA256).Hash -ne '4708a361a74f7e955635dbe3661cefb459bdc867423c3b1826a2c5a6ea4ac77d'){throw 'Unexpected AAR checksum'}
Copy-Item -LiteralPath $aar[0].FullName -Destination $androidLibDir -Force
