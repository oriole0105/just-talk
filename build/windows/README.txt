just-talk — Windows packaging notes
=====================================

Prerequisites
-------------
- Windows 10/11 (x86-64)
- Visual C++ Redistributable 2019+ (usually pre-installed)
- Microphone hardware
- For keyboard injection: no special requirements (uses SendInput)

Installation (manual)
---------------------
1. Download just-talk-x86_64-pc-windows-msvc.exe from the Releases page.
2. Copy to a folder in your PATH (e.g., C:\Users\<you>\bin\).
3. Rename to just-talk.exe if needed.
4. Create config file at:
     %APPDATA%\just-talk\config.toml
   (see README.md for config options)

Optional: auto-start
--------------------
To launch just-talk when you log in:
1. Press Win+R → shell:startup
2. Create a shortcut to just-talk.exe in the opened folder
3. Set "Start in" to the folder where just-talk.exe lives

WinGet packaging (future)
--------------------------
Planned: publish a WinGet manifest under
  manifests/j/just-talk/just-talk/<version>/
See: https://github.com/microsoft/winget-pkgs

NSIS / Inno Setup (future)
---------------------------
A full installer script will be added once the project reaches v1.0.
The installer will:
  - Copy just-talk.exe to %PROGRAMFILES%\just-talk\
  - Register an auto-start entry
  - Create a default config at %APPDATA%\just-talk\config.toml
