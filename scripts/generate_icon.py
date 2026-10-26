#!/usr/bin/env python3
import os
import math
import shutil
import subprocess
from PIL import Image, ImageDraw, ImageFilter

CANVAS_SIZE = 1024
SS = 4  # 4x supersampling for crisp Retina edges
SIZE = CANVAS_SIZE * SS

# Apple macOS HIG squircle dimensions at 1024x1024 (scaled by SS)
# Base squircle: 824x824, centered at x=100..924, y=100..924
BASE_W = int(824 * SS)
BASE_H = int(824 * SS)
BASE_X0 = int(100 * SS)
BASE_Y0 = int(100 * SS)
BASE_X1 = BASE_X0 + BASE_W
BASE_Y1 = BASE_Y0 + BASE_H
CORNER_RADIUS = int(185 * SS)

# Create 100% transparent canvas (macOS requires alpha=0 at outer corners to avoid system squircle jail)
canvas = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))

# 1. Soft downward ambient drop shadow (macOS HIG standard)
shadow = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))
s_draw = ImageDraw.Draw(shadow)
shadow_dy = int(16 * SS)
s_draw.rounded_rectangle(
    [BASE_X0, BASE_Y0 + shadow_dy, BASE_X1, BASE_Y1 + shadow_dy],
    radius=CORNER_RADIUS,
    fill=(0, 0, 0, 48)  # ~18.8% alpha ambient shadow
)
# Additional tighter contact shadow at bottom edge
s_draw.rounded_rectangle(
    [BASE_X0 + int(30 * SS), BASE_Y1 - int(10 * SS), BASE_X1 - int(30 * SS), BASE_Y1 + int(10 * SS)],
    radius=int(30 * SS),
    fill=(0, 0, 0, 24)
)
shadow = shadow.filter(ImageFilter.GaussianBlur(radius=int(18 * SS)))
canvas.paste(shadow, (0, 0), shadow)

# 2. Pure White Squircle Card with delicate hairline boundary stroke
squircle_mask = Image.new('L', (SIZE, SIZE), 0)
m_draw = ImageDraw.Draw(squircle_mask)
m_draw.rounded_rectangle(
    [BASE_X0, BASE_Y0, BASE_X1, BASE_Y1],
    radius=CORNER_RADIUS,
    fill=255
)

squircle_img = Image.new('RGBA', (SIZE, SIZE), (255, 255, 255, 255))
sq_draw = ImageDraw.Draw(squircle_img)
# 1px border at 1024 in subtle neutral gray rgba(0, 0, 0, 0.08)
sq_draw.rounded_rectangle(
    [BASE_X0, BASE_Y0, BASE_X1, BASE_Y1],
    radius=CORNER_RADIUS,
    outline=(0, 0, 0, 20),
    width=max(1, int(1.5 * SS))
)
canvas.paste(squircle_img, (0, 0), squircle_mask)

# 3. Terracotta-to-amber gradient Z glyph centered inside squircle
# Glyph size: ~500x500 at 1024 (~60% of squircle for Apple HIG optical breathing room)
glyph_size = int(500 * SS)
gx0 = BASE_X0 + (BASE_W - glyph_size) // 2
gy0 = BASE_Y0 + (BASE_H - glyph_size) // 2
gx1 = gx0 + glyph_size
gy1 = gy0 + glyph_size

glyph_mask = Image.new('L', (SIZE, SIZE), 0)
g_draw = ImageDraw.Draw(glyph_mask)

p_top = [
    (gx0, gy0),
    (gx1, gy0),
    (gx0 + glyph_size * 5 // 8, gy0 + glyph_size // 2),
    (gx0, gy0 + glyph_size // 2),
]
g_draw.polygon(p_top, fill=255)

p_bot = [
    (gx0 + glyph_size * 3 // 8, gy0 + glyph_size // 2),
    (gx1, gy0 + glyph_size // 2),
    (gx1, gy1),
    (gx0, gy1),
]
g_draw.polygon(p_bot, fill=255)

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
