// Разбор PNG — ровно настолько, чтобы прочитать скин игрока Java.
//
// Устройство файла — по спецификации PNG (W3C): подпись, куски IHDR, PLTE,
// tRNS, IDAT, IEND. Картинка в IDAT сжата zlib, каждая строка начинается с
// номера фильтра. Черезстрочная развёртка (Adam7) не поддерживается: скины
// с ней не сохраняют.

use std::io::Read;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Самая большая сторона картинки: скины — 64×64, у скинов высокой чёткости
/// бывает больше, но не больше этого.
const MAX_SIDE: u32 = 1024;

/// Картинка: ширина, высота и точки RGBA по строкам.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Разбирает PNG с глубиной 8 бит на канал; другое — `None`.
pub fn decode(bytes: &[u8]) -> Option<Image> {
    if bytes.get(..8)? != SIGNATURE {
        return None;
    }

    let mut at = 8;
    let (mut width, mut height, mut color_type) = (0u32, 0u32, 0u8);
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut alpha: Vec<u8> = Vec::new();
    let mut compressed = Vec::new();

    while at + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
        let kind = &bytes[at + 4..at + 8];
        let body = bytes.get(at + 8..at + 8 + length)?;
        at += 12 + length;

        match kind {
            b"IHDR" => {
                width = u32::from_be_bytes(body.get(0..4)?.try_into().ok()?);
                height = u32::from_be_bytes(body.get(4..8)?.try_into().ok()?);
                let depth = *body.get(8)?;
                color_type = *body.get(9)?;
                let interlace = *body.get(12)?;

                if depth != 8 || interlace != 0 || width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
                    return None;
                }
            }
            b"PLTE" => palette = body.as_chunks::<3>().0.to_vec(),
            b"tRNS" => alpha = body.to_vec(),
            b"IDAT" => compressed.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
    }

    let channels = match color_type {
        0 => 1, // серый
        2 => 3, // RGB
        3 => 1, // по палитре
        4 => 2, // серый с прозрачностью
        6 => 4, // RGBA
        _ => return None,
    };

    let stride = width as usize * channels;
    let expected = (stride + 1) * height as usize;
    let mut raw = Vec::with_capacity(expected);

    // Распаковывается не больше, чем нужно картинке: сжатые данные могут
    // развернуться во много раз больше заявленного.
    flate2::read::ZlibDecoder::new(&compressed[..]).take(expected as u64).read_to_end(&mut raw).ok()?;

    if raw.len() < expected {
        return None;
    }

    let pixels = unfilter(&raw, stride, height as usize, channels)?;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);

    for pixel in pixels.chunks_exact(channels) {
        match color_type {
            0 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]),
            2 => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
            3 => {
                let index = pixel[0] as usize;
                let [r, g, b] = *palette.get(index)?;
                rgba.extend_from_slice(&[r, g, b, alpha.get(index).copied().unwrap_or(255)]);
            }
            4 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]),
            _ => rgba.extend_from_slice(pixel),
        }
    }

    Some(Image { width, height, rgba })
}

/// Снимает фильтры строк: у каждой строки первый байт — номер фильтра.
fn unfilter(raw: &[u8], stride: usize, height: usize, step: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; stride * height];

    for row in 0..height {
        let filter = raw[row * (stride + 1)];
        let line = &raw[row * (stride + 1) + 1..(row + 1) * (stride + 1)];
        let (done, rest) = out.split_at_mut(row * stride);
        let above = if row == 0 { None } else { Some(&done[(row - 1) * stride..]) };
        let current = &mut rest[..stride];

        for i in 0..stride {
            let left = if i >= step { current[i - step] } else { 0 };
            let up = above.map_or(0, |a| a[i]);
            let corner = if i >= step { above.map_or(0, |a| a[i - step]) } else { 0 };

            let prediction = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((left as u16 + up as u16) / 2) as u8,
                4 => paeth(left, up, corner),
                _ => return None,
            };

            current[i] = line[i].wrapping_add(prediction);
        }
    }

    Some(out)
}

fn paeth(left: u8, up: u8, corner: u8) -> u8 {
    let p = left as i16 + up as i16 - corner as i16;
    let (to_left, to_up, to_corner) = ((p - left as i16).abs(), (p - up as i16).abs(), (p - corner as i16).abs());

    if to_left <= to_up && to_left <= to_corner {
        left
    } else if to_up <= to_corner {
        up
    } else {
        corner
    }
}

/// Собирает PNG из точек RGBA (8 бит на канал, без фильтров): так скин
/// игрока Bedrock уходит туда, где картинку ждут файлом. Контрольные суммы
/// настоящие — чужие разборщики их проверяют.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    use std::io::Write;

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(body);

        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out.extend_from_slice(&crc.sum().to_be_bytes());
    }

    let stride = width as usize * 4;
    let mut rows = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks(stride).take(height as usize) {
        rows.push(0); // строка без фильтра
        rows.extend_from_slice(row);
    }

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&rows).expect("сжатие в память не отказывает");
    let data = encoder.finish().expect("сжатие в память не отказывает");

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 бит, RGBA, без черезстрочности

    let mut out = SIGNATURE.to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &data);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Собирает PNG из готовых строк (с байтом фильтра впереди).
    fn build(width: u32, height: u32, color_type: u8, rows: &[u8]) -> Vec<u8> {
        fn chunk(out: &mut Vec<u8>, kind: &[u8], body: &[u8]) {
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(kind);
            out.extend_from_slice(body);
            out.extend_from_slice(&[0; 4]); // контрольная сумма не проверяется
        }

        let mut header = Vec::new();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, color_type, 0, 0, 0]);

        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(rows).expect("сжатие");
        let data = encoder.finish().expect("сжатие");

        let mut out = SIGNATURE.to_vec();
        chunk(&mut out, b"IHDR", &header);
        chunk(&mut out, b"IDAT", &data);
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn encoded_image_reads_back_with_true_checksums() {
        let rgba: Vec<u8> = (0..64 * 64 * 4).map(|at| (at * 7 % 251) as u8).collect();
        let bytes = encode(64, 64, &rgba);
        let image = decode(&bytes).expect("картинка");

        assert_eq!((image.width, image.height), (64, 64));
        assert_eq!(image.rgba, rgba);

        // CRC куска IHDR у любого PNG 64×64 RGBA одна и та же (посчитано zlib).
        assert_eq!(&bytes[29..33], &0xAA69_71DEu32.to_be_bytes());
    }

    #[test]
    fn rgba_with_filters_decodes() {
        // Две точки в строке, две строки: первая без фильтра, вторая — «сверху».
        let rows = [0, 10, 20, 30, 255, 1, 2, 3, 4, 2, 5, 5, 5, 0, 1, 1, 1, 1];
        let image = decode(&build(2, 2, 6, &rows)).expect("картинка");

        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(image.rgba, vec![10, 20, 30, 255, 1, 2, 3, 4, 15, 25, 35, 255, 2, 3, 4, 5]);
    }

    #[test]
    fn rgb_with_paeth_decodes() {
        let rows = [0, 100, 0, 0, 4, 1, 1, 1];
        let image = decode(&build(1, 2, 2, &rows)).expect("картинка");

        assert_eq!(image.rgba, vec![100, 0, 0, 255, 101, 1, 1, 255]);
    }

    #[test]
    fn not_png_is_rejected() {
        assert!(decode(b"GIF89a").is_none());
    }
}
