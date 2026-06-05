# Tray / app icons (generated, not committed as binaries here)

`tauri.conf.json` references the standard Tauri icon set in this folder:

```
icons/icon.ico            # Windows app + tray icon (referenced by app.trayIcon)
icons/32x32.png
icons/128x128.png
icons/128x128@2x.png
icons/icon.icns
```

These are **build artifacts**. Generate them from a single source PNG with the
Tauri CLI before building (do NOT hand-author binary blobs in source control):

```sh
pnpm tauri icon path/to/source-1024.png
```

## Status-dot tray icons (v2 plan §11.1)

The tray reflects health as green / yellow / red. Once the pipeline produces:

```
icons/icon_green.ico
icons/icon_yellow.ico
icons/icon_red.ico
```

wire them into `src/tray.rs::refresh` via `tray.set_icon(...)`. Until then the
status is conveyed through the tray tooltip (`정상` / `경고` / `에러`).

Without `icons/icon.ico` present, `cargo build` of the bundled app will fail at
the icon-embedding step — generate the set first.
