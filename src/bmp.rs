/// Very basic hand baked bmp parser/writer, works for our needs

const BMP_HEADER_SIZE: usize = 54;
/// buffer is assumed to be BGRX
pub fn save_bgrx_bmp(buffer: &[u8], ww: u32, hh: u32, name: &str) {
    let hh = hh as usize;
    let ww = ww as usize;

    let pad_per_line = ((4 - (ww * 3) % 4) % 4) as usize;
    let filesize = BMP_HEADER_SIZE + (3 * ww * hh) as usize + pad_per_line * hh as usize;

    let mut img = vec![0u8; filesize];

    for x in 0..ww {
        for inv_y in 0..hh {
            let y = (hh - 1) - inv_y;

            let b = buffer[(x + y * ww) * 4];
            let g = buffer[(x + y * ww) * 4 + 1];
            let r = buffer[(x + y * ww) * 4 + 2];

            let row_idx = BMP_HEADER_SIZE + y * ww * 3 + y * pad_per_line;
            let out_idx = row_idx + x * 3;
            img[out_idx + 2] = r as u8;
            img[out_idx + 1] = g as u8;
            img[out_idx + 0] = b as u8;
        }
    }
    save_rgb_bmp(&img, ww, hh, name);
}

pub fn save_rgb_bmp(buffer: &[u8], ww: usize, hh: usize, name: &str) {
    let pad_per_line = ((4 - (ww * 3) % 4) % 4) as usize;
    let filesize = 54 + (3 * ww * hh) as usize + pad_per_line * hh as usize;

    let mut bmpfileheader: [u8; 14] = [b'B', b'M', 0, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0];
    let bmpinfoheader_tmp: [u8; 16] = [40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 24, 0];
    let mut bmpinfoheader: [u8; 40] = [0; 40];
    bmpinfoheader[..16].copy_from_slice(&bmpinfoheader_tmp[..]);

    bmpfileheader[2] = filesize as u8;
    bmpfileheader[3] = (filesize >> 8) as u8;
    bmpfileheader[4] = (filesize >> 16) as u8;
    bmpfileheader[5] = (filesize >> 24) as u8;

    bmpinfoheader[4] = (ww) as u8;
    bmpinfoheader[5] = (ww >> 8) as u8;
    bmpinfoheader[6] = (ww >> 16) as u8;
    bmpinfoheader[7] = (ww >> 24) as u8;
    bmpinfoheader[8] = (hh) as u8;
    bmpinfoheader[9] = (hh >> 8) as u8;
    bmpinfoheader[10] = (hh >> 16) as u8;
    bmpinfoheader[11] = (hh >> 24) as u8;

    let mut f = std::fs::File::create(format!(".\\{}", name))
        .map_err(|err| (err, name))
        .unwrap();

    use std::io::Write;

    f.write_all(&bmpfileheader[..]).unwrap();
    f.write_all(&bmpinfoheader[..]).unwrap();

    let pad_per_line = ((4 - (ww * 3) % 4) % 4) as usize;
    let pad = [0u8; 64];
    for i in (0..hh).into_iter().rev() {
        f.write_all(&buffer[i * ww * 3..(i + 1) * ww * 3]).unwrap();
        f.write_all(&pad[..pad_per_line]).unwrap();
    }
}

pub fn save_gray_bmp(buffer: &[u8], ww: usize, hh: usize, name: &str) {
    // TODO: Instead of saving an RGB gray bmp actually save as gray?
    //       may need a 256 color palette that maps x -> (x,x,x)
    let buffer = &buffer
        .iter()
        .flat_map(|x| [*x, *x, *x].into_iter())
        .collect::<Vec<u8>>();
    save_rgb_bmp(buffer, ww, hh, name);
}

/// Very brittle handmade bmp parser
/// Returns (byte_data, width, height)
/// byte_data is packed, no padding, stride=width, order is R,G,B
pub fn parse_rgb_bmp(buffer: &[u8]) -> Result<(Vec<u8>, usize, usize), &'static str> {
    /*
    if true {
        let mut b = buffer;
        let i = bmp::from_reader(&mut b).unwrap();
        let (w, h) = (i.get_width() as usize, i.get_height() as usize);
        let mut out = Vec::with_capacity(3 * w * h);
        for y in 0..h {
            for x in 0..w {
                let bmp::Pixel { r, g, b } = i.get_pixel(x as u32, y as u32);
                out.push(r);
                out.push(g);
                out.push(b);
            }
        }
        return Ok((out, w, h));
    }
    */
    // Made using https://en.wikipedia.org/wiki/BMP_file_format for documentation

    // bmp header
    if !(*buffer.get(0).ok_or("no idx 0")? == 0x42 && *buffer.get(1).ok_or("no idx 1")? == 0x4D) {
        return Err("Error parsing header magic");
    }

    let range_to_u32 = |a, b| {
        Ok(u32::from_le_bytes(
            TryInto::<[u8; 4]>::try_into(buffer.get(a..b).ok_or("no idx")?)
                .ok()
                .ok_or("u32 conversion failed")?,
        ))
    };
    let range_to_u16 = |a, b| {
        Ok(u16::from_le_bytes(
            TryInto::<[u8; 2]>::try_into(buffer.get(a..b).ok_or("no idx")?)
                .ok()
                .ok_or("u16 conversion failed")?,
        ))
    };
    let range_to_usize_via_i32 = |a, b| {
        Ok(i32::from_le_bytes(
            TryInto::<[u8; 4]>::try_into(buffer.get(a..b).ok_or("no idx")?)
                .ok()
                .ok_or("i32 conversion failed")?,
        ) as u32 as usize)
    };

    let _file_size = range_to_u32(0x2, 0x6)?;
    // Could validate file_size == buffer.len()

    let reserved_1_and_2 = range_to_u32(0x6, 0x0A)?;
    if reserved_1_and_2 != 0 {
        return Err("reserved_1_and_2 are not 0");
    }

    let raw_data_start = range_to_u32(0x0A, 0x0E)? as usize;
    if raw_data_start >= buffer.len() {
        return Err("raw_data_start is beyond the end of the bmp");
    }

    // No support but anything but Windows BITMAPINFOHEADER
    let second_header_size = range_to_u32(0x0E, 0x12)?;
    if second_header_size != 40 {
        return Err("bmp is not of type Windows BITMAPINFOHEADER");
    }

    let width = range_to_usize_via_i32(0x12, 0x16)?;
    let height = range_to_usize_via_i32(0x16, 0x1A)?;

    let color_planes = range_to_u16(0x1A, 0x1C)?;
    if color_planes != 1 {
        return Err("color_planes is not 1");
    }

    // No support for anything but 24 bpp
    let bpp = range_to_u16(0x1C, 0x1E)?;
    if bpp != 24 {
        return Err("bpp is not 24");
    }

    // No compression support
    let compression = range_to_u32(0x1E, 0x22)?;
    if compression != 0 {
        return Err("compression is not 0");
    }

    let _raw_image_data_size = range_to_u32(0x22, 0x26)?;

    let _horiz_res = range_to_usize_via_i32(0x26, 0x2A)?;

    let _vert_res = range_to_usize_via_i32(0x2A, 0x2E)?;

    // No palette support
    let num_colors_in_palette = range_to_u32(0x2E, 0x32)?;
    if num_colors_in_palette != 0 {
        return Err("num_colors_in_palette is not 0");
    }

    // No idea what this is
    let num_important_colors = range_to_u32(0x32, 0x36)?;
    if num_important_colors != 0 {
        return Err("num_important_colors is not 0");
    }

    let raw_image_data = &buffer[raw_data_start..];
    let mut image_data = vec![0u8; width * height * 3];

    let pad_per_line = ((4 - (width * 3) % 4) % 4) as usize;
    let row_size = width * 3;
    let padded_row_size = row_size + pad_per_line;
    for y in 0..height {
        let out_row_start = row_size * (height - y - 1);
        let in_row_start = padded_row_size * y;
        for ([a, b, c], [c2, b2, a2]) in (&mut image_data
            [out_row_start..(out_row_start + row_size)])
            .array_chunks_mut::<3>()
            .zip(
                raw_image_data
                    .get(in_row_start..(in_row_start + row_size))
                    .ok_or("no raw idx")?
                    .array_chunks::<3>(),
            )
        {
            *a = *a2;
            *b = *b2;
            *c = *c2;
        }
    }

    Ok((image_data, width, height))
}

// const fn parser
// This made cargo check take 10+ minutes on a 7mb screenshot
// NOT worth it
// Requires staticvec = "0.11.0"
/*
pub const STATIC_RGB_VEC_LEN: usize = 1024 * 1024 * 7;
pub const STATIC_RGB_VEC_LEN_SMOL: usize = 1024 * 50;
/// Const version of `parse_rgb_bmp`
/// panics instead of returning a Result
/// use const_rgb_bmp_include_bytes! to use it on a file
pub const fn const_parse_rgb_bmp<const SIZE: usize>(
  buffer: &[u8],
) -> (staticvec::StaticVec<u8, SIZE>, usize, usize) {
  // Made using https://en.wikipedia.org/wiki/BMP_file_format for documentation
  const fn range_to_u32(a: usize, _b: usize, buffer: &[u8]) -> Result<u32, &'static str> {
    let buf = [buffer[a], buffer[a + 1], buffer[a + 2], buffer[a + 3]];
    Ok(u32::from_le_bytes(buf))
  }
  const fn range_to_u16(a: usize, _b: usize, buffer: &[u8]) -> Result<u16, &'static str> {
    let buf = [buffer[a], buffer[a + 1]];
    Ok(u16::from_le_bytes(buf))
  }
  const fn range_to_usize_via_i32(
    a: usize,
    _b: usize,
    buffer: &[u8],
  ) -> Result<usize, &'static str> {
    let buf = [buffer[a], buffer[a + 1], buffer[a + 2], buffer[a + 3]];
    Ok(i32::from_le_bytes(buf) as u32 as usize)
  }
  const fn unwrappy<T: Copy, U: Copy>(x: Result<T, U>) -> T {
    match x {
      Ok(x) => x,
      Err(_) => panic!(),
    }
  }

  // bmp header
  if !(buffer[0] == 0x42 && buffer[1] == 0x4D) {
    unwrappy(Result::<u8, &'static str>::Err(
      "Error parsing header magic",
    ));
  }

  let _file_size = unwrappy(range_to_u32(0x2, 0x6, buffer));
  // Could validate file_size == buffer.len()

  let reserved_1_and_2 = unwrappy(range_to_u32(0x6, 0x0A, buffer));
  if reserved_1_and_2 != 0 {
    unwrappy(Result::<u8, &'static str>::Err(
      "reserved_1_and_2 are not 0",
    ));
  }

  let raw_data_start = unwrappy(range_to_u32(0x0A, 0x0E, buffer)) as usize;
  if raw_data_start >= buffer.len() {
    unwrappy(Result::<u8, &'static str>::Err(
      "raw_data_start is beyond the end of the bmp",
    ));
  }

  // No support but anything but Windows BITMAPINFOHEADER
  let second_header_size = unwrappy(range_to_u32(0x0E, 0x12, buffer));
  if second_header_size != 40 {
    unwrappy(Result::<u8, &'static str>::Err(
      "bmp is not of type Windows BITMAPINFOHEADER",
    ));
  }

  let width = unwrappy(range_to_usize_via_i32(0x12, 0x16, buffer));
  let height = unwrappy(range_to_usize_via_i32(0x16, 0x1A, buffer));

  let color_planes = unwrappy(range_to_u16(0x1A, 0x1C, buffer));
  if color_planes != 1 {
    unwrappy(Result::<u8, &'static str>::Err("color_planes is not 1"));
  }

  // No support for anything but 24 bpp
  let bpp = unwrappy(range_to_u16(0x1C, 0x1E, buffer));
  if bpp != 24 {
    unwrappy(Result::<u8, &'static str>::Err("bpp is not 24"));
  }

  // No compression support
  let compression = unwrappy(range_to_u32(0x1E, 0x22, buffer));
  if compression != 0 {
    unwrappy(Result::<u8, &'static str>::Err("compression is not 0"));
  }

  let _raw_image_data_size = unwrappy(range_to_u32(0x22, 0x26, buffer));

  let _horiz_res = unwrappy(range_to_usize_via_i32(0x26, 0x2A, buffer));

  let _vert_res = unwrappy(range_to_usize_via_i32(0x2A, 0x2E, buffer));

  // No palette support
  let num_colors_in_palette = unwrappy(range_to_u32(0x2E, 0x32, buffer));
  if num_colors_in_palette != 0 {
    unwrappy(Result::<u8, &'static str>::Err(
      "num_colors_in_palette is not 0",
    ));
  }

  // No idea what this is
  let num_important_colors = unwrappy(range_to_u32(0x32, 0x36, buffer));
  if num_important_colors != 0 {
    unwrappy(Result::<u8, &'static str>::Err(
      "num_important_colors is not 0",
    ));
  }

  let mut image_data = staticvec::StaticVec::<u8, SIZE>::new();

  let pad_per_line = ((4 - (width * 3) % 4) % 4) as usize;
  let row_size = width * 3;
  let padded_row_size = row_size + pad_per_line;

  let mut y = height;
  while y > 0 {
    let in_row_start = raw_data_start + padded_row_size * (y - 1);

    let mut x = 0;
    while x < row_size {
      image_data.push(buffer[in_row_start + x + 2]);
      image_data.push(buffer[in_row_start + x + 1]);
      image_data.push(buffer[in_row_start + x + 0]);

      x += 3;
    }

    y -= 1;
  }

  (image_data, width, height)
}

macro_rules! const_rgb_bmp_include_bytes {
  ($file:expr) => {{
    const TUPLE: &'static (
      staticvec::StaticVec<u8, { bmp::STATIC_RGB_VEC_LEN }>,
      usize,
      usize,
    ) = &bmp::const_parse_rgb_bmp::<{ bmp::STATIC_RGB_VEC_LEN }>(include_bytes!($file));
    (TUPLE.0.as_slice(), TUPLE.1, TUPLE.2)
  }};
}

macro_rules! const_rgb_bmp_include_bytes_smol {
  ($file:expr) => {{
    const TUPLE: &'static (
      staticvec::StaticVec<u8, { bmp::STATIC_RGB_VEC_LEN_SMOL }>,
      usize,
      usize,
    ) = &bmp::const_parse_rgb_bmp::<{ bmp::STATIC_RGB_VEC_LEN_SMOL }>(include_bytes!($file));
    const W: usize = TUPLE.1;
    const H: usize = TUPLE.2;
    (TUPLE.0.as_slice(), W, H)
  }};
}

pub(crate) use const_rgb_bmp_include_bytes;
pub(crate) use const_rgb_bmp_include_bytes_smol;

*/
