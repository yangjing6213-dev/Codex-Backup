# ENHE Codex Backup icon

The app uses an AI-generated blue derivative of the company logo supplied by the project owner. It preserves the three slanted E bars and lightning silhouette, with a small local-backup tray. It does not imply OpenAI endorsement.

- Source: `desktop/src-tauri/icons/app-icon-source.png`.
- Desktop assets: the PNG/ICO/ICNS files in `desktop/src-tauri/icons/`.
- In-app and favicon asset: `desktop/public/app-icon.png`.
- Generated with the built-in imagegen tool; PNG/ICO sizes generated with the project's existing Tauri icon command. No extra runtime dependency.

Generation brief: one square Windows application icon; white ENHE E/lightning silhouette from the owner-supplied reference on a cobalt-blue rounded tile (#2563EB); small backup tray; readable at small sizes; no cloud, wordmark, slogan, watermark or 3D effects; transparent exterior. A subsequent automatic edge-cleanup variant was not selected because it did not improve the source at icon size.

Regenerate the desktop formats with `pnpm --dir desktop tauri icon src-tauri/icons/app-icon-source.png`; use the generated 128px PNG for `desktop/public/app-icon.png`. Only ship platform assets needed by this application.
