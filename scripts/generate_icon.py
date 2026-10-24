#!/usr/bin/env python3
import os
import math
from PIL import Image, ImageDraw, ImageFilter

CANVAS_SIZE = 1024

# 4x supersampling for ultra-crisp vector rendering
SS = 4
SIZE = CANVAS_SIZE * SS

img = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))

# macOS squircle specs at 1024:
# standard squircle bounds: pad=104 -> width=816
pad = int(104 * SS)
squircle_w = SIZE - 2 * pad
squircle_r = int(squircle_w * 0.223)

# 1. Subtle ambient drop shadow
shadow_img = Image.new('RGBA', (SIZE, SIZE), (0, 0, 0, 0))
shadow_draw = ImageDraw.Draw(shadow_img)
shadow_offset_y = int(14 * SS)
shadow_draw.rounded_rectangle(
    [pad, pad + shadow_offset_y, SIZE - pad, SIZE - pad + shadow_offset_y],
    radius=squircle_r,
    fill=(0, 0, 0, 48)
)
shadow_img = shadow_img.filter(ImageFilter.GaussianBlur(radius=int(20 * SS)))
img.paste(shadow_img, (0, 0), shadow_img)

# 2. Pure white squircle base
squircle_mask = Image.new('L', (SIZE, SIZE), 0)
s_draw = ImageDraw.Draw(squircle_mask)
s_draw.rounded_rectangle([pad, pad, SIZE - pad, SIZE - pad], radius=squircle_r, fill=255)

squircle_base = Image.new('RGBA', (SIZE, SIZE), (255, 255, 255, 255))
# Very subtle hairline edge (0.5pt macOS stroke)
base_draw = ImageDraw.Draw(squircle_base)
base_draw.rounded_rectangle([pad, pad, SIZE - pad, SIZE - pad], radius=squircle_r, outline=(0, 0, 0, 18), width=int(1.5 * SS))

# 3. Z Glyph bounds centered inside squircle
# Z glyph size: ~500x500 at 1024
glyph_size = int(500 * SS)
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

# Composite glyph onto white squircle
squircle_base.paste(grad_img, (0, 0), glyph_mask)

# Composite squircle onto final image with squircle_mask
img.paste(squircle_base, (0, 0), squircle_mask)

master_1024 = img.resize((CANVAS_SIZE, CANVAS_SIZE), Image.Resampling.LANCZOS)

# Generate iconset
iconset_dir = 'assets/icon.iconset'
os.makedirs(iconset_dir, exist_ok=True)

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

print("All iconset PNGs generated successfully.")
