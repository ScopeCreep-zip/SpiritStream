; SpiritStream NSIS Installer Hooks
; Handles legacy installation cleanup and user data migration during upgrades

!macro NSIS_HOOK_PREINSTALL
  ; ============================================================================
  ; PHASE 1: Clean up legacy PROGRAM FILES directories (side-by-side prevention)
  ; ============================================================================

  IfFileExists "$PROGRAMFILES\spirit-stream\*.*" 0 +2
    RMDir /r "$PROGRAMFILES\spirit-stream"
  IfFileExists "$PROGRAMFILES\spiritstream\*.*" 0 +2
    RMDir /r "$PROGRAMFILES\spiritstream"
  IfFileExists "$LOCALAPPDATA\Programs\spirit-stream\*.*" 0 +2
    RMDir /r "$LOCALAPPDATA\Programs\spirit-stream"
  IfFileExists "$LOCALAPPDATA\Programs\spiritstream\*.*" 0 +2
    RMDir /r "$LOCALAPPDATA\Programs\spiritstream"

  ; ============================================================================
  ; PHASE 2: Migrate USER DATA from legacy locations
  ; ============================================================================
  ; New location: %LOCALAPPDATA%\com.spiritstream.desktop\
  ; Old locations (checked in order):
  ;   1. %APPDATA%\SpiritStream\
  ;   2. %APPDATA%\spirit-stream\
  ;   3. %LOCALAPPDATA%\SpiritStream\
  ; ============================================================================

  ; Define the new data directory ($0)
  StrCpy $0 "$LOCALAPPDATA\com.spiritstream.desktop"

  ; Skip migration if new location already has profiles (don't overwrite existing data)
  IfFileExists "$0\profiles\*.*" skip_migration 0

  ; Try migrating from %APPDATA%\SpiritStream\ (most common legacy location)
  IfFileExists "$APPDATA\SpiritStream\profiles\*.*" 0 try_kebab_case
    DetailPrint "Found legacy data at $APPDATA\SpiritStream, migrating..."
    StrCpy $1 "$APPDATA\SpiritStream"
    Goto do_migration

  try_kebab_case:
  ; Try migrating from %APPDATA%\spirit-stream\
  IfFileExists "$APPDATA\spirit-stream\profiles\*.*" 0 try_localappdata
    DetailPrint "Found legacy data at $APPDATA\spirit-stream, migrating..."
    StrCpy $1 "$APPDATA\spirit-stream"
    Goto do_migration

  try_localappdata:
  ; Try migrating from %LOCALAPPDATA%\SpiritStream\
  IfFileExists "$LOCALAPPDATA\SpiritStream\profiles\*.*" 0 skip_migration
    DetailPrint "Found legacy data at $LOCALAPPDATA\SpiritStream, migrating..."
    StrCpy $1 "$LOCALAPPDATA\SpiritStream"
    Goto do_migration

  do_migration:
  ; $0 = new location, $1 = old location
  ; Create new directory structure
  CreateDirectory "$0"
  CreateDirectory "$0\profiles"
  CreateDirectory "$0\themes"
  CreateDirectory "$0\logs"
  CreateDirectory "$0\indexes"

  ; Copy profiles directory (critical user data)
  IfFileExists "$1\profiles\*.*" 0 +2
    CopyFiles /SILENT "$1\profiles\*.*" "$0\profiles"

  ; Copy settings.json
  IfFileExists "$1\settings.json" 0 +2
    CopyFiles /SILENT "$1\settings.json" "$0\settings.json"

  ; Copy machine encryption key (critical for encrypted profiles)
  IfFileExists "$1\.stream_key" 0 +2
    CopyFiles /SILENT "$1\.stream_key" "$0\.stream_key"

  ; Copy custom themes
  IfFileExists "$1\themes\*.*" 0 +2
    CopyFiles /SILENT "$1\themes\*.*" "$0\themes"

  ; Copy profile order indexes
  IfFileExists "$1\indexes\*.*" 0 +2
    CopyFiles /SILENT "$1\indexes\*.*" "$0\indexes"

  ; Create migration marker file
  FileOpen $2 "$0\.migrated_from_legacy" w
  FileWrite $2 "Migrated from: $1$\r$\n"
  FileWrite $2 "Migration date: ${__DATE__} ${__TIME__}$\r$\n"
  FileClose $2

  DetailPrint "User data migration completed successfully"

  skip_migration:
!macroend
