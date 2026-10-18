[Setup]
AppId={{5D2A7D1B-8D2C-4D4B-9C52-7B9A7C8D4E10}
AppName=Minecraft Server Downloader
AppVersion=0.0.1
AppPublisher=Minecraft-1314
VersionInfoVersion=0.0.1
DefaultDirName={autopf}\MinecraftServerDownloader
DefaultGroupName=Minecraft Server Downloader
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=MinecraftServerDownloader-Setup
SetupIconFile=..\app.ico
UninstallDisplayIcon={app}\app.ico
UninstallDisplayName=Minecraft Server Downloader
Uninstallable=yes
CreateUninstallRegKey=yes
WizardStyle=modern dynamic windows11
ShowLanguageDialog=yes
Compression=lzma2
SolidCompression=yes
CloseApplications=yes

[Languages]
Name: "zh"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\dist\MinecraftServerDownloader-win-x64\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\target\release\uninstall.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Minecraft Server Downloader"; Filename: "{app}\MinecraftServerDownloader.exe"
Name: "{group}\Uninstall Minecraft Server Downloader"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Minecraft Server Downloader"; Filename: "{app}\MinecraftServerDownloader.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\MinecraftServerDownloader.exe"; Description: "{cm:LaunchProgram,Minecraft Server Downloader}"; Flags: nowait postinstall skipifsilent
