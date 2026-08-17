#!/usr/bin/env python3
"""Rescayl background removal helper.

Requires: pip install rembg onnxruntime pillow
Usage: bg_removal.py <input> <output> <model>
The output extension decides the result:
  .jpg/.jpeg  -> composite onto a white background (no alpha)
  .webp       -> keep transparency (lossless WebP)
  anything else -> keep transparency (PNG)
"""
import io
import os
import sys


def main():
    if len(sys.argv) != 4:
        print("usage: bg_removal.py <input> <output> <model>", file=sys.stderr)
        return 2
    inp, outp, model = sys.argv[1], sys.argv[2], sys.argv[3]
    try:
        from rembg import new_session
        from rembg.bg import remove

        session = new_session(model)
        with open(inp, "rb") as f:
            data = f.read()
        result = remove(data, session=session)
        out_ext = os.path.splitext(outp)[1].lower()
        if out_ext in (".jpg", ".jpeg"):
            from PIL import Image

            img = Image.open(io.BytesIO(result)).convert("RGBA")
            bg = Image.new("RGBA", img.size, (255, 255, 255, 255))
            bg.alpha_composite(img)
            bg.convert("RGB").save(outp, "JPEG", quality=95)
        elif out_ext == ".webp":
            from PIL import Image

            img = Image.open(io.BytesIO(result)).convert("RGBA")
            img.save(outp, "WEBP", lossless=True)
        else:
            with open(outp, "wb") as f:
                f.write(result)
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
