#!/usr/bin/env bash
# macOS .app + DMG 打包（release.yml 的 macos-universal job 调用）。
# DMG 为标准安装视图：左侧 Cucumber.app、右侧 Applications 文件夹快捷方式、
# 背景引导图带箭头；内附「首次运行说明.txt」处理 Gatekeeper。
# 用法：macos-bundle.sh <universal二进制> <版本号> <输出目录>
set -euo pipefail
set -x

BIN_PATH="${1:?用法: macos-bundle.sh <universal二进制> <版本> <输出目录>}"
VER="${2:?缺版本号}"
DIST="${3:?缺输出目录}"
VOL="Cucumber"
APP_NAME="Cucumber.app"
BUNDLE_ID="io.github.chunshilaioqf.cucumber"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="$(mktemp -d)/vol"
trap 'hdiutil detach "/Volumes/${VOL}" >/dev/null 2>&1 || true' EXIT

# ---------- 1. 组装 .app ----------
APP_DIR="${STAGE}/${APP_NAME}"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"
cp "${BIN_PATH}" "${APP_DIR}/Contents/MacOS/cucumber"
chmod +x "${APP_DIR}/Contents/MacOS/cucumber"
cp "${ROOT}/assets/AppIcon.icns" "${APP_DIR}/Contents/Resources/AppIcon.icns"
cat > "${APP_DIR}/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>黄瓜</string>
    <key>CFBundleDisplayName</key>       <string>黄瓜</string>
    <key>CFBundleExecutable</key>        <string>cucumber</string>
    <key>CFBundleIdentifier</key>        <string>BUNDLE_ID_PLACEHOLDER</string>
    <key>CFBundleVersion</key>           <string>VER_PLACEHOLDER</string>
    <key>CFBundleShortVersionString</key><string>VER_PLACEHOLDER</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleIconFile</key>          <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>    <string>12.0</string>
    <key>LSUIElement</key>               <string>false</string>
    <key>NSHighResolutionCapable</key>   <true/>
    <key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
</dict>
</plist>
PLIST
sed -i '' "s/BUNDLE_ID_PLACEHOLDER/${BUNDLE_ID}/; s/VER_PLACEHOLDER/${VER}/g" "${APP_DIR}/Contents/Info.plist"
codesign --force --deep --sign - "${APP_DIR}" >/dev/null 2>&1 || true

# ---------- 2. 组装卷内容：Applications 快捷方式 + 背景 + 首次运行说明 ----------
ln -s /Applications "${STAGE}/Applications"
mkdir -p "${STAGE}/.background"
cp "${ROOT}/assets/dmg_bg.png" "${STAGE}/.background/dmg_bg.png"
cat > "${STAGE}/首次运行说明.txt" <<'README'
黄瓜 cucumber —— 首次运行说明

1. 把左侧的「Cucumber」图标拖到右侧的 Applications 文件夹，完成安装。
2. 在「启动台」或「应用程序」里打开 黄瓜（Cucumber）。
3. 若 macOS 提示「无法验证开发者」：

   macOS 13 及更早：在提示上右键 →「打开」→ 再点「打开」；
   macOS 14/15（Sonoma/Sequoia）：打开 系统设置 → 隐私与安全性，
   页面下拉到「安全性」一栏，找到关于 Cucumber 的提示 → 点「仍要打开」。

   或在「终端」执行一条命令后直接打开：

     xattr -dr com.apple.quarantine /Applications/Cucumber.app

   （本应用未购买 Apple Developer 证书做签名公证，这是无证书分发的
   正常提示；有证书后此提示会消失。）

项目主页：https://github.com/chunshilaioqf/cucumber
README

# ---------- 3. 可写 DMG → Finder 布局 → 压缩 ----------
RW="$(mktemp -u "${DIST}/rw.XXXXXX").dmg"
hdiutil create -volname "${VOL}" -size 48m -fs HFS+ -attach "${RW}" -quiet
# 拷贝进挂载卷（含隐藏文件）
cp -R "${STAGE}/" "/Volumes/${VOL}/"
osascript <<OSA || echo "::warning::Finder 布局失败，退化为默认视图 DMG"
tell application "Finder"
  tell disk "${VOL}"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {0, 0, 660, 400}
    set theViewOptions to the icon view options of container window
    set arrangement of theViewOptions to not arranged
    set icon size of theViewOptions to 128
    set background picture of theViewOptions to file ".background:dmg_bg.png"
    set position of item "Cucumber" of container window to {116, 126}
    set position of item "Applications" of container window to {416, 126}
    close
    open
    update without registering applications
    delay 2
  end tell
end tell
OSA
for i in 1 2 3; do
  hdiutil detach "/Volumes/${VOL}" -quiet && break
  sleep 2
done
sleep 1

DMG="${DIST}/cucumber-${VER}-macos-universal.dmg"
rm -f "${DMG}"
hdiutil convert "${RW}" -format UDZO -o "${DMG}" -quiet
rm -f "${RW}"
echo "DMG_OK=${DMG}"
