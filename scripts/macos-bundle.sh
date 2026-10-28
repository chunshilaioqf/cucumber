#!/usr/bin/env bash
# macOS .app + DMG 打包（release.yml 的 macos-universal job 调用）。
# 用法：macos-bundle.sh <universal二进制> <版本号> <输出目录>
set -euo pipefail

BIN_PATH="${1:?用法: macos-bundle.sh <universal二进制> <版本> <输出目录>}"
VER="${2:?缺版本号}"
DIST="${3:?缺输出目录}"
APP_NAME="Cucumber.app"
BUNDLE_ID="io.github.chunshilaioqf.cucumber"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

APP_DIR="${DIST}/${APP_NAME}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"

# 二进制
cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/cucumber"
chmod +x "${APP_DIR}/Contents/MacOS/cucumber"

# 图标
cp "${ROOT}/assets/AppIcon.icns" "${APP_DIR}/Contents/Resources/AppIcon.icns"

# Info.plist：双击即启动 GUI（LSUIElement=false，有 Dock 图标与菜单栏）
cat > "${APP_DIR}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>黄瓜</string>
    <key>CFBundleDisplayName</key>       <string>黄瓜</string>
    <key>CFBundleExecutable</key>        <string>cucumber</string>
    <key>CFBundleIdentifier</key>        <string>${BUNDLE_ID}</string>
    <key>CFBundleVersion</key>           <string>${VER}</string>
    <key>CFBundleShortVersionString</key><string>${VER}</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleIconFile</key>          <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>    <string>12.0</string>
    <key>LSUIElement</key>               <string>false</string>
    <key>NSHighResolutionCapable</key>   <true/>
    <key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
</dict>
</plist>
PLIST

# 对 .app 做本地签名（ad-hoc，无证书也可运行；正式分发可后续替换 Developer ID）
codesign --force --deep --sign - "${APP_DIR}" >/dev/null 2>&1 || true

# DMG：UDZO 压缩，卷名带版本
DMG="${DIST}/cucumber-${VER}-macos-universal.dmg"
hdiutil create -volname "黄瓜 cucumber ${VER}" \
    -srcfolder "${APP_DIR}" \
    -ov -format UDZO "${DMG}" -quiet
echo "DMG_OK=${DMG}"
