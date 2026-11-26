#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:?Set ANDROID_HOME to the Android SDK}}"
tools="$sdk/build-tools/34.0.0"
platform="$sdk/platforms/android-34/android.jar"
target="${CARGO_TARGET_DIR:-target}"
key="${ANDROID_USER_HOME:-$HOME/.android}/debug.keystore"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

zxing="${VISALES_ZXING_JAR:-$work/zxing.jar}"
if [[ ! -f "$zxing" ]]; then
    curl --fail --silent --show-error --location \
        https://repo.maven.apache.org/maven2/com/google/zxing/core/3.5.3/core-3.5.3.jar -o "$zxing"
fi
printf '%s  %s\n' 8d8064c1636fdaef7189dd9055c7d59950a8940a12f2293956446ec3c109fd82 "$zxing" | sha256sum --check --status

mkdir "$work/classes"
javac --release 8 -classpath "$platform:$zxing" -d "$work/classes" android/ScannerActivity.java android/Updater.java
jar cf "$work/scanner.jar" -C "$work/classes" .
"$tools/d8" --min-api 26 --lib "$platform" --output "$work" "$work/scanner.jar" "$zxing"

cargo apk build --lib
apk="$target/debug/apk/ViSales.apk"
python3 - "$apk" "$work/classes.dex" "$work/unsigned.apk" <<'PY'
import shutil
import sys
import zipfile

apk, dex, output = sys.argv[1:]
with zipfile.ZipFile(apk) as source, zipfile.ZipFile(output, 'w') as destination:
    for entry in source.infolist():
        if entry.filename.startswith('META-INF/'):
            continue
        with source.open(entry) as src, destination.open(entry, 'w') as dst:
            shutil.copyfileobj(src, dst)
    destination.write('android/LICENSE-ZXing', 'assets/LICENSE-ZXing.txt')
    destination.write(dex, 'classes.dex', compress_type=zipfile.ZIP_DEFLATED)
PY
"$tools/zipalign" -f 4 "$work/unsigned.apk" "$work/aligned.apk"
"$tools/apksigner" sign --ks "$key" --ks-pass pass:android --key-pass pass:android \
    --out "$work/ViSales.apk" "$work/aligned.apk"
"$tools/apksigner" verify "$work/ViSales.apk"
cp "$work/ViSales.apk" "$apk"
