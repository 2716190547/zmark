#!/usr/bin/env python3
import os
import math
from PIL import Image, ImageDraw, ImageFilter

CANVAS_SIZE = 1024

# 4x supersampling for ultra-crisp vector rendering
SS = 4
SIZE = CANVAS_SIZE * SS

img = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))

# Full-bleed solid white canvas across the entire 1024x1024 frame.
# By making the icon full bleed (zero outer transparency / zero inner card margin),
# macOS LaunchServices / Dock applies its single native system squircle mask.
# This completely eliminates the "double squircle / nested card" jail effect.
img = Image.new('RGBA', (SIZE, SIZE), (255, 255, 255, 255))

# 2. Z Glyph bounds centered inside full-bleed canvas
# Glyph size: 560x560 at 1024x1024 (scaled by SS)
glyph_size = int(560 * SS)
gx0 = (SIZE - glyph_size) // 2
gy0 = (SIZE - glyph_size) // 2
gx1 = gx0 + glyph_size
gy1 = gy0 + glyph_size

# Create glyph mask
glyph_mask = Image.new('L', (SIZE, SIZE), 0)
g_draw = ImageDraw.Draw(glyph_mask)

# Top polygon: (gx0, gy0) -> (gx1, gy0) -> (gx0 + 5/8 * w, gy0 + 1/2 * h) -> (gx0, gy0 + 1/2 * h)
p_top = [
    (gx0, gy0),
    (gx1, gy0),
    (gx0 + glyph_size * 5 // 8, gy0 + glyph_size // 2),
    (gx0, gy0 + glyph_size // 2),
]
g_draw.polygon(p_top, fill=255)

# Bottom polygon: (gx0 + 3/8 * w, gy0 + 1/2 * h) -> (gx1, gy0 + 1/2 * h) -> (gx1, gy1) -> (gx0, gy1)
p_bot = [
    (gx0 + glyph_size * 3 // 8, gy0 + glyph_size // 2),
    (gx1, gy0 + glyph_size // 2),
    (gx1, gy1),
    (gx0, gy1),
]
g_draw.polygon(p_bot, fill=255)

# 4. Generate diagonal linear gradient: warm terracotta red-brown to golden yellow
grad_img = Image.new('RGBA', (SIZE, SIZE))
grad_pix = grad_img.load()

c_start = (194, 65, 40)   # Terracotta red-brown #C24128
c_end = (245, 158, 11)    # Golden amber #F59E0B

for y in range(gy0, gy1):
    for x in range(gx0, gx1):
        t = ((x - gx0) + (y - gy0)) / (2.0 * glyph_size)
        t = max(0.0, min(1.0, t))
        t_smooth = (1.0 - math.cos(t * math.pi)) / 2.0
        r = int(c_start[0] + (c_end[0] - c_start[0]) * t_smooth)
        g = int(c_start[1] + (c_end[1] - c_start[1]) * t_smooth)
        b = int(c_start[2] + (c_end[2] - c_start[2]) * t_smooth)
        grad_pix[x, y] = (r, g, b, 255)

# Composite glyph directly onto full-bleed white canvas
img.paste(grad_img, (0, 0), glyph_mask)

master_1024 = img.resize((CANVAS_SIZE, CANVAS_SIZE), Image.Resampling.LANCZOS)

# Generate iconset and xcassets
iconset_dir = 'assets/icon.iconset'
os.makedirs(iconset_dir, exist_ok=True)

xcassets_dir = 'assets/Assets.xcassets/AppIcon.appiconset'
os.makedirs(xcassets_dir, exist_ok=True)

sizes = [
    (16, 'icon_16x16.png'),
    (32, 'icon_16x16@2x.png'),
    (32, 'icon_32x32.png'),
    (64, 'icon_32x32@2x.png'),
    (128, 'icon_128x128.png'),
    (256, 'icon_128x128@2x.png'),
    (256, 'icon_256x256.png'),
    (512, 'icon_256x256@2x.png'),
    (512, 'icon_512x512.png'),
    (1024, 'icon_512x512@2x.png'),
]

for sz, fname in sizes:
    resized = master_1024.resize((sz, sz), Image.Resampling.LANCZOS)
    resized.save(os.path.join(iconset_dir, fname))
    resized.save(os.path.join(xcassets_dir, fname))

import subprocess
import shutil

# Try iconutil
try:
    subprocess.run(['iconutil', '-c', 'icns', iconset_dir, '-o', 'assets/AppIcon.icns'], check=True)
    print("iconutil compiled AppIcon.icns successfully.")
except Exception as e:
    print(f"iconutil skipped or failed: {e}")

# Try actool
try:
    tmp_out = 'assets/build_car_tmp'
    os.makedirs(tmp_out, exist_ok=True)
    subprocess.run([
        '/usr/bin/actool', 'assets/Assets.xcassets',
        '--compile', tmp_out,
        '--platform', 'macosx',
        '--minimum-deployment-target', '12.0',
        '--app-icon', 'AppIcon',
        '--output-partial-info-plist', os.path.join(tmp_out, 'partial.plist')
    ], check=True)
    if os.path.exists(os.path.join(tmp_out, 'Assets.car')):
        shutil.copy(os.path.join(tmp_out, 'Assets.car'), 'assets/Assets.car')
        print("actool compiled Assets.car successfully.")
    shutil.rmtree(tmp_out, ignore_errors=True)
except Exception as e:
    print(f"actool skipped or failed: {e}")

print("All icon assets generated successfully.")
