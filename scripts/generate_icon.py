#!/usr/bin/env python3
import os
import math
from PIL import Image, ImageDraw

CANVAS_SIZE = 1024
SS = 4  # 4x supersampling for ultra-crisp edges
SIZE = CANVAS_SIZE * SS

# 1. Full-bleed pure white canvas: solid #FFFFFF across entire 1024x1024 frame.
# This eliminates any inner floating squircle and lets macOS Dock clip it
# cleanly to the native single-layer Dock tile with zero nesting ("一层套一层").
canvas = Image.new('RGBA', (SIZE, SIZE), (255, 255, 255, 255))

# 2. Render warm terracotta-to-amber gradient Z glyph centered inside canvas
# Glyph size: ~620x620 at 1024x1024 (scaled by SS) for bold, clear brand recognition
glyph_size = int(620 * SS)
gx0 = (SIZE - glyph_size) // 2
gy0 = (SIZE - glyph_size) // 2
gx1 = gx0 + glyph_size
gy1 = gy0 + glyph_size

# Create glyph mask
glyph_mask = Image.new('L', (SIZE, SIZE), 0)
g_draw = ImageDraw.Draw(glyph_mask)

# Top polygon of Z:
p_top = [
    (gx0, gy0),
    (gx1, gy0),
    (gx0 + glyph_size * 5 // 8, gy0 + glyph_size // 2),
    (gx0, gy0 + glyph_size // 2),
]
g_draw.polygon(p_top, fill=255)

# Bottom polygon of Z:
p_bot = [
    (gx0 + glyph_size * 3 // 8, gy0 + glyph_size // 2),
    (gx1, gy0 + glyph_size // 2),
    (gx1, gy1),
    (gx0, gy1),
]
g_draw.polygon(p_bot, fill=255)

# Generate diagonal linear gradient: warm terracotta red-brown to golden yellow
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

# Composite gradient glyph onto white canvas
canvas.paste(grad_img, (0, 0), glyph_mask)

# Downscale to master 1024x1024 using Lanczos filter
master_1024 = canvas.resize((CANVAS_SIZE, CANVAS_SIZE), Image.Resampling.LANCZOS)

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

# Try iconutil to build AppIcon.icns
try:
    subprocess.run(['iconutil', '-c', 'icns', iconset_dir, '-o', 'assets/AppIcon.icns'], check=True)
    print("Successfully built assets/AppIcon.icns")
except Exception as e:
    print(f"Warning: iconutil failed: {e}")

# Try actool if Xcode tools are available to build Assets.car
if shutil.which('actool'):
    try:
        subprocess.run([
            'actool',
            'assets/Assets.xcassets',
            '--compile', 'assets',
            '--platform', 'macosx',
            '--minimum-deployment-target', '13.0',
            '--app-icon', 'AppIcon',
            '--output-partial-info-plist', '/tmp/appicon_partial.plist'
        ], check=True)
        print("Successfully compiled assets/Assets.car via actool")
    except Exception as e:
        print(f"actool notice: {e}")
