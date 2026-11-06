# OSD font sheets

The goggles' OSD fonts are PNG images with the glyphs in columns. The size of the image gives its layout.

| Image width | Image height | Layout |
|-------------|--------------|--------|
| cell width × 1 | cell height × 256 or 512 | One page, 256 or 512 characters (INAV uses 512) |
| cell width × 2, 3 or 4 | cell height × 256 | Two, three or four colour pages (Betaflight 4.5 uses four) |

Cell sizes, matched to the video height:

| Video height | Cell (width × height) |
|--------------|-----------------------|
| 540 p | 18 × 27 |
| 720 p | 24 × 36 |
| 1080 p | 36 × 54 |
| 1440 p | 48 × 72 |
| 2160 p | 72 × 108 |

Pages are side by side. Glyph `page × characters_per_page + character` is the character `character` of colour
page `page`. A sheet of another size is resized to the cell size of the video once, before drawing.

The detection order matters: a 36-pixel-wide sheet can be one 36-pixel cell or two 18-pixel cells. The
candidate is accepted only when its character count is a multiple of 256.
