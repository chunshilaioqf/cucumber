; 黄瓜 cucumber — Windows Inno Setup 安装脚本（任务 3）
; CI 由 release.yml 调用 iscc 编译；产物 cucumber-<ver>-windows-<arch>-setup.exe
;
; 命令行 /D 传入：Ver（版本） ArchLabel（x86_64/arm64） BinDir（exe 目录）

#ifndef Ver
#define Ver "0.0.0"
#endif
#ifndef ArchLabel
#define ArchLabel "x86_64"
#endif
#ifndef BinDir
#define BinDir "target\release"
#endif

[Setup]
AppId={{8E1C4B2A-9F3D-4C7B-A6E5-CUCUMBER42MGR}
AppName=黄瓜 cucumber
AppVersion={#Ver}
AppPublisher=chunshilaioqf
AppPublisherURL=https://github.com/chunshilaioqf/cucumber
AppSupportURL=https://github.com/chunshilaioqf/cucumber/issues
DefaultDirName={autopf}\Cucumber
DefaultGroupName=黄瓜 cucumber
UninstallDisplayName=黄瓜 cucumber {#Ver}
OutputBaseFilename=cucumber-{#Ver}-windows-{#ArchLabel}-setup
OutputDir=dist
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ShowLanguageDialog=yes

[Languages]
Name: "zh"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "autostart"; Description: "开机自动启动"; Flags: unchecked

[Files]
Source: "{#BinDir}\cucumber.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\黄瓜 cucumber"; Filename: "{app}\cucumber.exe"
Name: "{group}\{cm:UninstallProgram,黄瓜 cucumber}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\黄瓜 cucumber"; Filename: "{app}\cucumber.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Cucumber"; ValueData: """{app}\cucumber.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\cucumber.exe"; Description: "立即启动 黄瓜 cucumber"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
