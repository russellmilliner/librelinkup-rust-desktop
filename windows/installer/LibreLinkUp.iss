; Inno Setup script for LibreLinkUp Rust Desktop (Windows).
; Build:  ISCC.exe /DAppVersion=0.4.2 /DSourceExe=..\target\x86_64-pc-windows-gnu\release\LibreLinkUp.exe LibreLinkUp.iss

#ifndef AppVersion
  #define AppVersion "0.4.2"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\x86_64-pc-windows-gnu\release\LibreLinkUp.exe"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

#define AppName "LibreLinkUp Rust Desktop"
#define AppExe "LibreLinkUp.exe"
#define AppUserModelId "io.github.russellmilliner.LibreLinkUpDesktop"
#define AppUrl "https://github.com/russellmilliner/librelinkup-rust-desktop"

[Setup]
AppId={{6F1E2C3A-8B4D-4E5F-9A7B-2C1D0E3F4A5B}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Russell Milliner
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup
; Per-user install by default (no admin prompt); admins may choose "all users".
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=LibreLinkUp-Setup-{#AppVersion}-x64
SetupIconFile=..\assets\app.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
; Close a running instance (Restart Manager) before replacing files.
CloseApplications=force
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "autostart"; Description: "Start {#AppName} when I sign in to Windows (minimized to the tray)"; GroupDescription: "Startup:"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.md"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\..\NOTICE"; DestDir: "{app}"; DestName: "NOTICE.txt"; Flags: ignoreversion

[Icons]
; The AppUserModelID on the Start Menu shortcut ties Windows toast notifications to the app.
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelId}"; Comment: "Blood glucose monitor for LibreLinkUp"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; AppUserModelID: "{#AppUserModelId}"; Tasks: desktopicon

[Registry]
; Login autostart (same value the app's "Start at Windows sign-in" setting manages).
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "LibreLinkUpDesktop"; ValueData: """{app}\{#AppExe}"" --minimized"; Flags: uninsdeletevalue; Tasks: autostart
; Toast notification identity (also written by the app at startup); removed on uninstall.
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\{#AppUserModelId}"; ValueType: string; ValueName: "DisplayName"; ValueData: "{#AppName}"; Flags: uninsdeletekey

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#AppExe} /F"; Flags: runhidden; RunOnceId: "StopLibreLinkUp"

[UninstallDelete]
; Remove the autostart value even if it was set from inside the app.
Type: files; Name: "{app}\*.log"

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'LibreLinkUpDesktop');
    if MsgBox('Also delete your LibreLinkUp settings and encrypted sign-in vault?' + #13#10 +
              '(' + ExpandConstant('{userappdata}\LibreLinkUpDesktop') + ')',
              mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
    begin
      DelTree(ExpandConstant('{userappdata}\LibreLinkUpDesktop'), True, True, True);
      DelTree(ExpandConstant('{localappdata}\LibreLinkUpDesktop'), True, True, True);
    end;
  end;
end;
