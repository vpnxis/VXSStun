$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Set-Location $projectRoot
if(!$env:ANDROID_HOME -or !$env:JAVA_HOME){throw 'Set ANDROID_HOME and JAVA_HOME (JDK 17)'}
$ndkRoot = $env:ANDROID_NDK_HOME
if(!$ndkRoot){$ndkRoot = Join-Path $env:ANDROID_HOME 'ndk/27.0.12077973'}
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $ndkRoot 'toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android26-clang.cmd'
$env:CARGO_TARGET_DIR = Join-Path $projectRoot '.build/rust-android'
$env:GRADLE_USER_HOME = Join-Path $projectRoot '.build/gradle-home'
$env:ANDROID_USER_HOME = Join-Path $projectRoot '.build/android-user'
$env:npm_config_cache = Join-Path $projectRoot '.build/npm-cache'
$env:TEMP = Join-Path $projectRoot '.build/tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force -Path $env:TEMP,$env:ANDROID_USER_HOME | Out-Null
$env:Path = "$env:JAVA_HOME/bin;$env:Path"
& npm.cmd run build
if($LASTEXITCODE){throw 'Frontend build failed'}
& cargo build --locked --manifest-path crates/profiles/Cargo.toml --target aarch64-linux-android --release
if($LASTEXITCODE){throw 'ARM64 parser build failed'}
$native = Join-Path $projectRoot 'android/app/src/main/jniLibs/arm64-v8a'
$web = Join-Path $projectRoot 'android/app/src/main/assets/web'
if(Test-Path -LiteralPath $web){
    $resolvedWeb=[IO.Path]::GetFullPath($web)
    $expectedWeb=[IO.Path]::GetFullPath((Join-Path $projectRoot 'android/app/src/main/assets/web'))
    if($resolvedWeb -ne $expectedWeb -or !(Test-Path -LiteralPath (Join-Path $web 'index.html'))){throw 'Unexpected generated web asset directory'}
    $previousWeb=Join-Path $projectRoot ('.build/web-before-'+[guid]::NewGuid().ToString('N'))
    Move-Item -LiteralPath $resolvedWeb -Destination $previousWeb
}
New-Item -ItemType Directory -Force -Path $native,$web | Out-Null
Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'aarch64-linux-android/release/libvxsstun_profiles.so') -Destination $native -Force
Copy-Item -Path 'dist/*' -Destination $web -Recurse -Force
& .\android\gradlew.bat -p android --no-daemon :app:assembleDebug :app:testDebugUnitTest
if($LASTEXITCODE){throw 'Android build failed'}
New-Item -ItemType Directory -Force -Path artifacts | Out-Null
Copy-Item -LiteralPath 'android/app/build/outputs/apk/debug/app-debug.apk' -Destination 'artifacts/VXSStun-0.0.1.alpha.1-arm64.apk' -Force
