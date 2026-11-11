; The Vidcue Windows installer: a per-user setup (no administrator rights needed) that
; installs vidcue.exe with its bundled libmpv-2.dll, a Start-menu icon and an optional
; desktop shortcut. The release workflow calls it as:
;   ISCC.exe /DAppVersion=0.2.0-alpha.13 /DFileVersion=0.2.0 tools\windows\vidcue.iss
; with dist\vidcue.exe and dist\libmpv-2.dll in place. Paths are relative to the
; repository root (SourceDir); the installer is written to installer\.

#ifndef AppVersion
#define AppVersion "0.0.0"
#endif
#ifndef FileVersion
#define FileVersion "0.0.0"
#endif

[Setup]
SourceDir=..\..
AppId={{7C1E3B9A-52D4-4A6E-9B8F-0A6D3E2C4D51}
AppName=Vidcue
AppVersion={#AppVersion}
VersionInfoVersion={#FileVersion}
AppPublisher=Vidcue
AppPublisherURL=https://github.com/Vanir20342222/vidcue
DefaultDirName={localappdata}\Programs\Vidcue
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=installer
OutputBaseFilename=VidcueSetup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\vidcue.exe

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "dist\vidcue.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "dist\libmpv-2.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Vidcue"; Filename: "{app}\vidcue.exe"
Name: "{autodesktop}\Vidcue"; Filename: "{app}\vidcue.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\vidcue.exe"; Description: "{cm:LaunchProgram,Vidcue}"; Flags: nowait postinstall skipifsilent

; Settings, queues, history and downloads stay on uninstall.
