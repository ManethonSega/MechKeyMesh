"""Package the approved logo without altering its artwork (requires Pillow)."""

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "apps/desktop/frontend/dist/assets/logo.png"
ICONS = ROOT / "apps/desktop/src-tauri/icons"
SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)
PNG_SIZES = {"32x32.png": 32, "128x128.png": 128, "128x128@2x.png": 256}


def square_icon(master, size):
    fitted = ImageOps.contain(master, (size, size), Image.Resampling.LANCZOS)
    result = Image.new("RGBA", (size, size))
    result.paste(fitted, ((size - fitted.width) // 2, (size - fitted.height) // 2))
    return result


def verify(master):
    checksum = (MASTER.parent / "logo.sha256").read_text().strip().split()[0]
    assert hashlib.sha256(MASTER.read_bytes()).hexdigest() == checksum, "Master logo changed"
    alpha = master.getchannel("A")
    assert alpha.getextrema() == (0, 255), "Logo must have transparent surroundings"
    assert all(alpha.getpixel(p) == 0 for p in (
        (0, 0), (master.width - 1, 0),
        (0, master.height - 1), (master.width - 1, master.height - 1),
    )), "White background returned"
    for name, size in PNG_SIZES.items():
        with Image.open(ICONS / name) as actual:
            expected = square_icon(master, size)
            assert actual.mode == "RGBA" and actual.size == (size, size), name
            assert actual.tobytes() == expected.tobytes(), f"Wrong artwork: {name}"
    with Image.open(ICONS / "icon.ico") as ico:
        assert ico.ico.sizes() == {(s, s) for s in SIZES}, "Missing ICO sizes"
        for size in SIZES:
            actual = ico.ico.getimage((size, size)).convert("RGBA")
            assert actual.tobytes() == square_icon(master, size).tobytes(), f"Wrong ICO: {size}"
    frontend = ROOT / "apps/desktop/frontend/dist"
    html = (frontend / "index.html").read_text()
    assert html.count('src="assets/logo.png"') == 2, "Splash/sidebar logo binding"
    assert 'href="assets/logo.png"' in html, "Favicon binding"
    assert not (frontend / "assets/logo.svg").exists(), "Obsolete logo remains"
    config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text())
    expected_paths = [f"icons/{name}" for name in PNG_SIZES] + ["icons/icon.ico"]
    assert config["bundle"]["icon"] == expected_paths, "Window/executable icon binding"
    tray = (ROOT / "apps/desktop/src-tauri/src/background.rs").read_text()
    assert 'include_bytes!("../icons/32x32.png")' in tray, "Tray logo binding"
    print("Transparent original logo checksum, UI bindings and all 12 icon variants verified.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    with Image.open(MASTER) as source:
        master = source.convert("RGBA")
    if not args.check:
        ICONS.mkdir(parents=True, exist_ok=True)
        for name, size in PNG_SIZES.items():
            square_icon(master, size).save(ICONS / name)
        # Supply exact per-size resamples, rather than resampling a 256px derivative.
        square_icon(master, 256).save(
            ICONS / "icon.ico", format="ICO", sizes=[(s, s) for s in SIZES],
            append_images=[square_icon(master, s) for s in SIZES if s != 256],
        )
    verify(master)


if __name__ == "__main__":
    main()
