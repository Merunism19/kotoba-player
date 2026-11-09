; NSIS installer hooks (tauri.conf.json bundle.windows.nsis.installerHooks).
;
; Always put a Kotoba Player shortcut on the desktop (owner request, 2026-11-08). Tauri's template
; only offers it as a checkbox on the finish page. CreateOrUpdateDesktopShortcut is the template's
; own function: it skips updater runs (/UPDATE) and /NS (no shortcuts), and the uninstaller
; already removes the shortcut.
!macro NSIS_HOOK_POSTINSTALL
  Call CreateOrUpdateDesktopShortcut
!macroend
