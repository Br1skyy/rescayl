<div align="center">

<img src="renderer/public/icon.png" width="200" />

# Rescayl

**Free and Open Source AI Image Upscaler**

</div>

## What is Rescayl?

Rescayl is a free, open source AI image upscaler built on Tauri. It takes your
low resolution images and makes them bigger while preserving (and even enhancing)
the details, using real AI models running on your GPU.

It is a community-driven project, forked from
[Upscayl](https://github.com/upscayl/upscayl) and rebuilt from the ground up
with a Rust backend and a modern web frontend.

## Why does this exist?

Upscayl was a great idea, but it stopped getting small updates and now focuses on large feature additions at once. Features
stalled, bugs lingered, and the tool didn't fit my use case. I wanted to keep pushing the project forward, so I forked it.

We also have a strong distain for Electron. The original Upscayl ships a
full Chromium browser alongside the app, which means a 200MB+ download for
something that could be 20MB. Tauri gives us a native Rust backend with a
small, fast webview that uses the system browser instead of bundling one.
The result is a dramatically smaller app that starts faster and uses much less
memory.

## What makes Rescayl different?

### Native background removal

Rescayl includes built in AI background removal, powered by the ONNX Runtime
running directly in Rust.

The segmentation model (ISNet) is bundled with the app, so it works offline
with zero setup.

### Faster release cycle

We ship features as they are ready, not on a quarterly schedule. Bug fixes,
new models, and UI improvements land when they are done.

### Smaller footprint

Tauri means no bundled browser. Rescayl is a fraction of the size of
Electron-based alternatives while looking and feeling better.

### Full control

The Rust backend is fully open source. You know whats going on under the hood now.

## Supported platforms

- Linux
- Windows
- macOS

## How it works

Rescayl uses AI models (Real-ESRGAN and others, via NCNN + Vulkan) to
enhance your images by inferring plausible high-resolution detail from
low-resolution input. Your GPU does the heavy lifting.

Background removal uses an ONNX segmentation model (ISNet) that runs
entirely in the Tauri backend via the `ort` crate. No external runtimes
are required.

## Models

Rescayl ships with several pre-installed models, each tuned for different
content types:

| Model | Scale | Size | Best for |
|-------|-------|------|----------|
| High Fidelity | 4x | 32 MB | General purpose, photos, mixed content |
| Standard | 4x | 32 MB | Photos and natural images |
| Ultrasharp | 4x | 32 MB | Maximum detail and edge sharpness |
| Ultramix | 4x | 32 MB | Balanced sharpness and natural look |
| Remacri | 4x | 32 MB | Nature and portrait photography |
| Digital Art | 4x | 8.6 MB | Illustrations, paintings, flat colors |
| Anime Video | 2x/3x/4x | 1.2 MB | Anime, cartoons, stylized video |
| Lite | 4x | 2.4 MB | Fast previews, low-end GPUs |

## FAQ

**Can Rescayl de-blur my images?**
Rescayl can enhance low-resolution and pixelated images, but it cannot
fix focus blur or motion blur.

**Why is my GPU not working?**
NCNN Vulkan requires a Vulkan-compatible GPU. Most integrated GPUs and
CPUs will not work. Try selecting a different GPU ID in settings.

**Does background removal require Python?**
No. Background removal runs entirely in Rust using ONNX Runtime. Just
enable it and go.

## Development

Prerequisites:

- [Node.js](https://nodejs.org) (18+)
- [Rust](https://rustup.rs) (stable)
- Tauri v2 system dependencies for your OS (see the
  [Tauri prerequisites guide](https://tauri.app/start/prerequisites/))

```bash
# Install npm dependencies
npm install

# Run in development mode
WEBKIT_DISABLE_DMABUF_RENDERER=1 npx tauri dev
```

The frontend is a Next.js static site. The backend is Rust (Tauri) and
drives `upscayl-bin` (Real-ESRGAN/NCNN) as a subprocess for upscaling,
and the `ort` crate for background removal.

### Binary download

Rescayl downloads platform-specific `upscayl-bin` binaries separately.
Place them at `resources/<os>/bin/upscayl-bin` (where `<os>` is `linux`,
`mac`, or `win`) before packaging. See `update_upscayl_ncnn_binaries.sh`
for the download helper.

## License

[AGPL-3.0](./LICENSE)
