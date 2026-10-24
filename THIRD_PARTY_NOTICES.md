# Third-party notices

This repository vendors the following third-party code. Each component remains under its own licence; the MIT licence in [`LICENSE`](LICENSE) covers only the original 256.app code.

| Component | Location | Licence | Upstream |
|-----------|----------|---------|----------|
| `@vladmandic/face-api` 1.7.15 (library + tiny face detector, 68-point landmark and recognition model weights) | `static/vendor/face-api/` | MIT | https://github.com/vladmandic/face-api |
| TensorFlow.js (bundled inside `face-api.min.js`) | `static/vendor/face-api/face-api.min.js` | Apache-2.0 | https://github.com/tensorflow/tfjs |

Rust and npm dependencies are fetched at build time under their own licences (see `Cargo.lock` and `e2e/package.json`).
