$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$artifactDir = Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null
$destination = Join-Path $artifactDir 'VXSStun-0.0.1.alpha.1-source.zip'
# Allowlist only. No caches, native runtime binaries, user profiles, APKs or signing keys.
$files = @()
foreach($name in @('README.md','LICENSE','LICENSE-jsQR.txt','THIRD_PARTY.md','TEST_REPORT.md','SECURITY.md','.gitignore','package.json','package-lock.json','index.html','vite.config.js')) {$files += Get-Item -LiteralPath (Join-Path $projectRoot $name)}
foreach($folder in @('src','tests','scripts','crates/profiles/src','crates/profiles/tests','src-tauri/src','src-tauri/icons','src-tauri/capabilities','android/gradle','android/app/src/main/java','android/app/src/main/res','android/app/src/test')) {$files += Get-ChildItem -LiteralPath (Join-Path $projectRoot $folder) -Recurse -File}
foreach($name in @('crates/profiles/Cargo.toml','crates/profiles/Cargo.lock','crates/profiles/build.rs','src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/build.rs','src-tauri/tauri.conf.json','android/gradlew','android/gradlew.bat','android/gradle.properties','android/settings.gradle','android/build.gradle','android/app/build.gradle','android/app/src/main/AndroidManifest.xml')) {$files += Get-Item -LiteralPath (Join-Path $projectRoot $name)}
$files += Get-ChildItem -LiteralPath (Join-Path $projectRoot 'src-tauri/runtime') -File | Where-Object {$_.Extension -in '.json','.txt'}
$files += Get-ChildItem -LiteralPath (Join-Path $projectRoot 'android/app/src/main/assets/licenses') -File
Add-Type -AssemblyName System.IO.Compression
$stream = [IO.File]::Open($destination,[IO.FileMode]::Create)
$zip = [IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create)
try {
    foreach($file in ($files | Sort-Object FullName -Unique)) {
        $relative = $file.FullName.Substring($projectRoot.Length+1).Replace('\','/')
        if($relative -match '(^|/)(\.build|target|node_modules|build|jniLibs)(/|$)|\.(jks|keystore|dpapi|apk|aar|exe|dll)$'){throw "Unexpected source file: $relative"}
        $entry=$zip.CreateEntry('VXSStun/'+$relative,[IO.Compression.CompressionLevel]::Optimal)
        $inputStream=$file.OpenRead();$outputStream=$entry.Open()
        try{$inputStream.CopyTo($outputStream)}finally{$inputStream.Dispose();$outputStream.Dispose()}
    }
}finally{$zip.Dispose();$stream.Dispose()}
Get-FileHash -LiteralPath $destination -Algorithm SHA256
