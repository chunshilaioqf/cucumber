# 黄瓜 cucumber RPM spec（CI 中由 rpmbuild -bb 构建）
Name: cucumber
Version: %{ver}
Release: 1%{?dist}
Summary: 进程与服务统一管理工具（黄瓜）
License: MIT
URL: https://github.com/chunshilaioqf/cucumber
BuildArch: %{arch}
AutoReqProv: no
Requires: /bin/sh

%description
黄瓜（cucumber）：跨平台本地进程与服务统一管理工具。
仪表盘（CPU/GPU/内存/磁盘/网络）、进程 CRUD 与命令执行、日志四格式过滤分页、
本地 HTTP/WS API 与 MCP 接口、Skill 安装、系统托盘、应用内更新。

%install
%{__install} -D -m 0755 %{bin} %{buildroot}/usr/bin/cucumber
%{__install} -D -m 0644 %{desktop} %{buildroot}/usr/share/applications/cucumber.desktop
for s in 48 128 256 512; do
  %{__install} -D -m 0644 %{icons}/icon_${s}.png %{buildroot}/usr/share/icons/hicolor/${s}x${s}/apps/cucumber.png
done

%files
/usr/bin/cucumber
/usr/share/applications/cucumber.desktop
/usr/share/icons/hicolor/*/apps/cucumber.png

%changelog
* 自动构建 CI release
- 黄瓜 cucumber 发布版
