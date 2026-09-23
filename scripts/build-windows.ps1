$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location $projectRoot
$env:CARGO_TARGET_DIR = Join-Path $projectRoot '.build/windows'
$env:npm_config_cache = Join-Path $projectRoot '.build/npm-cache'
$env:TEMP = Join-Path $projectRoot '.build/tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP | Out-Null
$pins=Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/runtime/manifest.json') -Raw | ConvertFrom-Json
foreach($name in @('xray.exe','wintun.dll')) {
    if((Get-FileHash -LiteralPath (Join-Path $projectRoot "src-tauri/runtime/$name") -Algorithm SHA256).Hash.ToLowerInvariant() -ne $pins.$name){throw "Runtime checksum mismatch: $name. Run prepare-runtime.ps1"}
}
& npm.cmd run build
if($LASTEXITCODE){throw 'Frontend build failed'}
& npm.cmd test
if($LASTEXITCODE){throw 'Frontend tests failed'}
# The standalone cdylib/rlib tests and Tauri have different unified dependency
# features. Keep their artifact directories separate, including on repeat builds.
& cargo test --locked --manifest-path crates/profiles/Cargo.toml --target-dir (Join-Path $projectRoot '.build/profile-tests')
if($LASTEXITCODE){throw 'Profile tests failed'}
$env:VXSSTUN_XRAY=Join-Path $projectRoot 'src-tauri/runtime/xray.exe'
& cargo test --locked --manifest-path crates/profiles/Cargo.toml --target-dir (Join-Path $projectRoot '.build/profile-tests') --test engine_config -- --ignored
if($LASTEXITCODE){throw 'Runtime configuration tests failed'}
& cargo test --locked --manifest-path src-tauri/Cargo.toml
if($LASTEXITCODE){throw 'Desktop tests failed'}
& cargo build --locked --manifest-path src-tauri/Cargo.toml --release --features custom-protocol
if($LASTEXITCODE){throw 'EXE build failed'}
$output = Join-Path $projectRoot 'artifacts/VXSStun-0.0.1.alpha.1-windows-x64'
New-Item -ItemType Directory -Force -Path $output,(Join-Path $output 'runtime') | Out-Null
Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/vxsstun.exe') -Destination (Join-Path $output 'VXSStun.exe') -Force
foreach($name in @('xray.exe','wintun.dll','manifest.json','LICENSE-Xray.txt','LICENSE-Wintun.txt')){Copy-Item -LiteralPath (Join-Path $projectRoot "src-tauri/runtime/$name") -Destination (Join-Path $output 'runtime') -Force}
Copy-Item -LiteralPath README.md,LICENSE,LICENSE-jsQR.txt,THIRD_PARTY.md,TEST_REPORT.md,SECURITY.md -Destination $output -Force
Compress-Archive -LiteralPath $output -DestinationPath (Join-Path $projectRoot 'artifacts/VXSStun-0.0.1.alpha.1-windows-x64.zip') -Force
