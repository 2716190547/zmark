#!/usr/bin/env python3
import os
import math
import shutil
import subprocess
from PIL import Image, ImageDraw

CANVAS_SIZE = 1024
SS = 4
SIZE = CANVAS_SIZE * SS

# 1. Generate modern macOS Tahoe Icon Composer (.icon) bundle
# macOS 27 Tahoe mandates IconImageStack (.icon) with background fill.
# Without IconImageStack, macOS forces apps into "Squircle Jail" (intrusive gray container).
icon_dir = 'assets/AppIcon.icon'
assets_dir = os.path.join(icon_dir, 'Assets')
os.makedirs(assets_dir, exist_ok=True)

# Render crisp vector Z glyph at 4x supersampling (4096 -> 1024)
glyph_canvas = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))

# Glyph size: ~560 at 1024 (balanced breathing room inside squircle safe area)
g_size = int(560 * SS)
gx0 = (SIZE - g_size) // 2
gy0 = (SIZE - g_size) // 2
gx1 = gx0 + g_size
gy1 = gy0 + g_size

mask = Image.new('L', (SIZE, SIZE), 0)
d = ImageDraw.Draw(mask)
p_top = [
    (gx0, gy0),
    (gx1, gy0),
    (gx0 + g_size * 5 // 8, gy0 + g_size // 2),
    (gx0, gy0 + g_size // 2),
]
d.polygon(p_top, fill=255)

p_bot = [
    (gx0 + g_size * 3 // 8, gy0 + g_size // 2),
    (gx1, gy0 + g_size // 2),
    (gx1, gy1),
    (gx0, gy1),
]
d.polygon(p_bot, fill=255)

# Diagonal gradient: Terracotta red-brown #C24128 -> Golden amber #F59E0B
grad_img = Image.new('RGBA', (SIZE, SIZE))
grad_pix = grad_img.load()
c_start = (194, 65, 40)
c_end = (245, 158, 11)

for y in range(gy0, gy1):
    for x in range(gx0, gx1):
        t = ((x - gx0) + (y - gy0)) / (2.0 * g_size)
        t = max(0.0, min(1.0, t))
        t_smooth = (1.0 - math.cos(t * math.pi)) / 2.0
        r = int(c_start[0] + (c_end[0] - c_start[0]) * t_smooth)
        g = int(c_start[1] + (c_end[1] - c_start[1]) * t_smooth)
        b = int(c_start[2] + (c_end[2] - c_start[2]) * t_smooth)
        grad_pix[x, y] = (r, g, b, 255)

glyph_canvas.paste(grad_img, (0, 0), mask)
glyph_1024 = glyph_canvas.resize((CANVAS_SIZE, CANVAS_SIZE), Image.Resampling.LANCZOS)
glyph_1024.save(os.path.join(assets_dir, 'glyph.png'))

# 2. Write Apple Icon Composer manifest
# fill defines the Squircle tile color directly, eliminating any gray fallback background
icon_json = '''{
  "fill" : {
    "automatic-gradient" : "extended-srgb:1.00000,1.00000,1.00000,1.00000"
  },
  "groups" : [
    {
      "layers" : [
        {
          "image-name" : "glyph.png"
        }
      ]
    }
  ],
  "supported-platforms" : {
    "squares" : [
      "macOS"
    ]
  }
}'''

with open(os.path.join(icon_dir, 'icon.json'), 'w') as f:
    f.write(icon_json)

# 3. Compile modern IconImageStack Assets.car and AppIcon.icns via actool
if shutil.which('actool'):
    try:
        subprocess.run([
            'actool',
            icon_dir,
            '--compile', 'assets',
            '--platform', 'macosx',
            '--minimum-deployment-target', '13.0',
            '--app-icon', 'AppIcon',
            '--output-partial-info-plist', '/tmp/appicon_partial.plist'
        ], check=True)
        print("Successfully compiled modern IconImageStack Assets.car and AppIcon.icns via actool.")
    except Exception as e:
        print(f"actool compilation error: {e}")

print("Icon assets successfully updated.")
