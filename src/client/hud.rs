use font8x8::{BASIC_FONTS, UnicodeFonts};

const WORD_SCALE: u32 = 5;

pub fn rasterize_word_texture(word: &str, letter_colors: &[[u8; 4]]) -> (Vec<u8>, u32, u32) {
    let cleaned = if word.is_empty() { "waiting" } else { word };
    let chars: Vec<char> = cleaned.chars().collect();
    let glyph_count = chars.len().max(1) as u32;
    let glyph_w = 8 * WORD_SCALE;
    let glyph_h = 8 * WORD_SCALE;
    let spacing = WORD_SCALE;
    let width = glyph_count * glyph_w + glyph_count.saturating_sub(1) * spacing;
    let height = glyph_h;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for (i, c) in chars.iter().enumerate() {
        let glyph = BASIC_FONTS
            .get(*c)
            .or_else(|| BASIC_FONTS.get(c.to_ascii_lowercase()));
        let Some(bitmap) = glyph else {
            continue;
        };
        let color = letter_colors
            .get(i)
            .copied()
            .unwrap_or([245, 232, 112, 255]);
        let base_x = i as u32 * (glyph_w + spacing);
        for (row, bits) in bitmap.iter().enumerate() {
            for col in 0..8u32 {
                if ((bits >> col) & 1) == 0 {
                    continue;
                }
                for sy in 0..WORD_SCALE {
                    for sx in 0..WORD_SCALE {
                        let x = base_x + col * WORD_SCALE + sx;
                        let y = row as u32 * WORD_SCALE + sy;
                        let idx = ((y * width + x) * 4) as usize;
                        pixels[idx] = color[0];
                        pixels[idx + 1] = color[1];
                        pixels[idx + 2] = color[2];
                        pixels[idx + 3] = color[3];
                    }
                }
            }
        }
    }

    (pixels, width.max(1), height.max(1))
}

pub fn rasterize_multiline_text(
    lines: &[(String, [u8; 4])],
    scale: u32,
    char_spacing: u32,
    line_gap: u32,
) -> (Vec<u8>, u32, u32) {
    if lines.is_empty() {
        return (vec![0, 0, 0, 0], 1, 1);
    }
    let glyph_w = 8 * scale;
    let glyph_h = 8 * scale;
    let max_chars = lines
        .iter()
        .map(|(line, _)| line.chars().count() as u32)
        .max()
        .unwrap_or(1)
        .max(1);
    let width = max_chars * glyph_w + max_chars.saturating_sub(1) * char_spacing;
    let height = lines.len() as u32 * glyph_h + (lines.len() as u32 - 1) * line_gap;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for (line_idx, (line, color)) in lines.iter().enumerate() {
        let y_base = line_idx as u32 * (glyph_h + line_gap);
        for (i, c) in line.chars().enumerate() {
            let glyph = BASIC_FONTS
                .get(c)
                .or_else(|| BASIC_FONTS.get(c.to_ascii_lowercase()));
            let Some(bitmap) = glyph else {
                continue;
            };
            let base_x = i as u32 * (glyph_w + char_spacing);
            for (row, bits) in bitmap.iter().enumerate() {
                for col in 0..8u32 {
                    if ((bits >> col) & 1) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let x = base_x + col * scale + sx;
                            let y = y_base + row as u32 * scale + sy;
                            let idx = ((y * width + x) * 4) as usize;
                            pixels[idx] = color[0];
                            pixels[idx + 1] = color[1];
                            pixels[idx + 2] = color[2];
                            pixels[idx + 3] = color[3];
                        }
                    }
                }
            }
        }
    }

    (pixels, width.max(1), height.max(1))
}
