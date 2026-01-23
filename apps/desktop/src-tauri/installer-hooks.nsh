!macro NSIS_HOOK_PREINSTALL
  ; Remove legacy SpiritStream install folders so upgrades don't install side-by-side.
  IfFileExists "$PROGRAMFILES\\spirit-stream\\*.*" 0 +2
    RMDir /r "$PROGRAMFILES\\spirit-stream"
  IfFileExists "$PROGRAMFILES\\spiritstream\\*.*" 0 +2
    RMDir /r "$PROGRAMFILES\\spiritstream"
  IfFileExists "$LOCALAPPDATA\\Programs\\spirit-stream\\*.*" 0 +2
    RMDir /r "$LOCALAPPDATA\\Programs\\spirit-stream"
  IfFileExists "$LOCALAPPDATA\\Programs\\spiritstream\\*.*" 0 +2
    RMDir /r "$LOCALAPPDATA\\Programs\\spiritstream"
!macroend
