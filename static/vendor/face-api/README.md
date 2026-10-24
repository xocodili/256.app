# `@vladmandic/face-api` — vendored

This is a frozen copy of `@vladmandic/face-api@1.7.15` (library + the three model groups used
by the face-capture onboarding step: tiny face detector, 68-point landmarks, and 128-D
recognition embedder).

## Why it's vendored

Face capture is on the **critical onboarding path** (`/onboarding` → `POST /me/face-capture`,
see `src/routes/social.rs::face_capture`). When this script or the model `.bin` shards fail
to load, the user sees:

> Could not load face detection models. Check your connection.

…and cannot finish onboarding. Reasons a public CDN can fail for an individual user — even
when the rest of the site loads — include:

- Network-level blocking of `cdn.jsdelivr.net` (corp firewall, regional restrictions, school
  Wi-Fi filtering).
- Browser extension / ad-blocker rules that match the CDN path.
- TLS-inspecting antivirus mangling the binary `.bin` shards.
- jsdelivr / npm CDN incidents.

Self-hosting moves the assets to the same origin as the app (`/static/vendor/face-api/...`)
and removes the third-party dependency from the onboarding hot path.

## Files

| File | Purpose | Size |
|------|---------|------|
| `face-api.min.js` | Library — exposes the `faceapi` global. | ~1.3 MB |
| `model/tiny_face_detector_model-weights_manifest.json` + `.bin` | Detector (`TinyFaceDetectorOptions`). | ~200 KB |
| `model/face_landmark_68_model-weights_manifest.json` + `.bin` | 68-point landmarks (used for yaw check). | ~360 KB |
| `model/face_recognition_model-weights_manifest.json` + `.bin` | 128-D descriptor (used as the stored embedding). | ~6.4 MB |

Total: ~8.3 MB.

## Refreshing / upgrading

```powershell
$base = "https://cdn.jsdelivr.net/npm/@vladmandic/face-api@<VERSION>"
Invoke-WebRequest "$base/dist/face-api.min.js" -OutFile "face-api.min.js"
foreach ($m in @("tiny_face_detector_model","face_landmark_68_model","face_recognition_model")) {
    Invoke-WebRequest "$base/model/$m-weights_manifest.json" -OutFile "model/$m-weights_manifest.json"
    Invoke-WebRequest "$base/model/$m.bin" -OutFile "model/$m.bin"
}
```

When bumping the version, also update:

- `static/js/face-capture.js` — `EMBEDDING_MODEL_ID` constant (so stored `user_faces` rows
  carry the right provenance tag).
- The version mention in this README.

Upstream: <https://github.com/vladmandic/face-api>.
