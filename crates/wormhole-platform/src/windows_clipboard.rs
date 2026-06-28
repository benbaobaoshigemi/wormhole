use anyhow::{anyhow, Context, Result};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use std::{ffi::c_void, io::Cursor, ptr};
use windows_sys::Win32::{
    Foundation::{HGLOBAL, HWND},
    Graphics::Gdi::{BI_BITFIELDS, BI_RGB},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
            OpenClipboard, SetClipboardData,
        },
        Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
    },
};
use wormhole_core::ClipboardPort;

const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;

pub struct SystemClipboard;

impl SystemClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl ClipboardPort for SystemClipboard {
    fn read_text(&mut self) -> Result<Option<String>> {
        with_clipboard(|| unsafe {
            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return Ok(None);
            }
            let handle = GetClipboardData(CF_UNICODETEXT);
            if handle.is_null() {
                return Ok(None);
            }
            let ptr = GlobalLock(handle as HGLOBAL) as *const u16;
            if ptr.is_null() {
                return Ok(None);
            }
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            GlobalUnlock(handle as HGLOBAL);
            Ok(Some(text))
        })
    }

    fn write_text(&mut self, text: &str) -> Result<()> {
        with_clipboard(|| unsafe {
            EmptyClipboard();
            let wide = text
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let bytes = wide.len() * std::mem::size_of::<u16>();
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if handle.is_null() {
                return Err(anyhow!("GlobalAlloc failed for CF_UNICODETEXT"));
            }
            let ptr = GlobalLock(handle) as *mut u8;
            if ptr.is_null() {
                return Err(anyhow!("GlobalLock failed for CF_UNICODETEXT"));
            }
            ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr, bytes);
            GlobalUnlock(handle);
            if SetClipboardData(CF_UNICODETEXT, handle as *mut c_void).is_null() {
                return Err(anyhow!("SetClipboardData failed for CF_UNICODETEXT"));
            }
            Ok(())
        })
    }

    fn read_png(&mut self) -> Result<Option<Vec<u8>>> {
        with_clipboard(|| unsafe {
            if IsClipboardFormatAvailable(CF_DIB) == 0 {
                return Ok(None);
            }
            let handle = GetClipboardData(CF_DIB);
            if handle.is_null() {
                return Ok(None);
            }
            let ptr = GlobalLock(handle as HGLOBAL) as *const u8;
            if ptr.is_null() {
                return Ok(None);
            }
            let size = GlobalSize(handle as HGLOBAL);
            let dib = std::slice::from_raw_parts(ptr, size);
            let png = dib_to_png(dib);
            GlobalUnlock(handle as HGLOBAL);
            png.map(Some)
        })
    }

    fn write_png(&mut self, png: &[u8]) -> Result<()> {
        let dib = png_to_dib(png)?;
        with_clipboard(|| unsafe {
            EmptyClipboard();
            let handle = GlobalAlloc(GMEM_MOVEABLE, dib.len());
            if handle.is_null() {
                return Err(anyhow!("GlobalAlloc failed for CF_DIB"));
            }
            let ptr = GlobalLock(handle) as *mut u8;
            if ptr.is_null() {
                return Err(anyhow!("GlobalLock failed for CF_DIB"));
            }
            ptr::copy_nonoverlapping(dib.as_ptr(), ptr, dib.len());
            GlobalUnlock(handle);
            if SetClipboardData(CF_DIB, handle as *mut c_void).is_null() {
                return Err(anyhow!("SetClipboardData failed for CF_DIB"));
            }
            Ok(())
        })
    }
}

fn with_clipboard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    unsafe {
        if OpenClipboard(0 as HWND) == 0 {
            return Err(anyhow!("OpenClipboard failed"));
        }
    }
    let result = f();
    unsafe {
        CloseClipboard();
    }
    result
}

fn dib_to_png(dib: &[u8]) -> Result<Vec<u8>> {
    let header = DibHeader::parse(dib)?;
    let width = header.width.unsigned_abs();
    let height = header.height.unsigned_abs();
    let pixel_offset = dib_pixel_offset(dib, &header)?;
    let stride = dib_stride(width, header.bit_count);
    let needed = pixel_offset + stride * height as usize;
    if dib.len() < needed {
        return Err(anyhow!("malformed CF_DIB bitmap data"));
    }
    let top_down = header.height < 0;
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let source_y = if top_down { y } else { height - 1 - y };
        let row = pixel_offset + source_y as usize * stride;
        for x in 0..width as usize {
            match header.bit_count {
                24 => {
                    let p = row + x * 3;
                    rgba.extend_from_slice(&[dib[p + 2], dib[p + 1], dib[p], 255]);
                }
                32 => {
                    let p = row + x * 4;
                    let alpha = if dib[p + 3] == 0 { 255 } else { dib[p + 3] };
                    rgba.extend_from_slice(&[dib[p + 2], dib[p + 1], dib[p], alpha]);
                }
                _ => return Err(anyhow!("unsupported CF_DIB bit depth {}", header.bit_count)),
            }
        }
    }
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba)
        .context("build RGBA image from CF_DIB")?;
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(image)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .context("encode CF_DIB as PNG")?;
    Ok(png)
}

fn png_to_dib(png: &[u8]) -> Result<Vec<u8>> {
    let image = image::load_from_memory_with_format(png, ImageFormat::Png)
        .context("decode PNG clipboard payload")?
        .to_rgba8();
    let width = image.width();
    let height = image.height();
    let stride = width as usize * 4;
    let mut dib = Vec::with_capacity(40 + stride * height as usize);
    write_u32(&mut dib, 40);
    write_i32(&mut dib, width as i32);
    write_i32(&mut dib, height as i32);
    write_u16(&mut dib, 1);
    write_u16(&mut dib, 32);
    write_u32(&mut dib, BI_RGB);
    write_u32(&mut dib, (stride * height as usize) as u32);
    write_i32(&mut dib, 2835);
    write_i32(&mut dib, 2835);
    write_u32(&mut dib, 0);
    write_u32(&mut dib, 0);
    for y in (0..height).rev() {
        for x in 0..width {
            let p = image.get_pixel(x, y).0;
            dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    Ok(dib)
}

#[derive(Debug)]
struct DibHeader {
    size: u32,
    width: i32,
    height: i32,
    bit_count: u16,
    compression: u32,
    colors_used: u32,
}

impl DibHeader {
    fn parse(dib: &[u8]) -> Result<Self> {
        if dib.len() < 40 {
            return Err(anyhow!("CF_DIB header too small"));
        }
        let size = read_u32(dib, 0);
        if size < 40 || dib.len() < size as usize {
            return Err(anyhow!("unsupported CF_DIB header size {}", size));
        }
        let planes = read_u16(dib, 12);
        let bit_count = read_u16(dib, 14);
        let compression = read_u32(dib, 16);
        if planes != 1 {
            return Err(anyhow!("invalid CF_DIB planes {}", planes));
        }
        if bit_count != 24 && bit_count != 32 {
            return Err(anyhow!("unsupported CF_DIB bit depth {}", bit_count));
        }
        if compression != BI_RGB && compression != BI_BITFIELDS {
            return Err(anyhow!("unsupported CF_DIB compression {}", compression));
        }
        Ok(Self {
            size,
            width: read_i32(dib, 4),
            height: read_i32(dib, 8),
            bit_count,
            compression,
            colors_used: read_u32(dib, 32),
        })
    }
}

fn dib_pixel_offset(dib: &[u8], header: &DibHeader) -> Result<usize> {
    let mut offset = header.size as usize;
    if header.compression == BI_BITFIELDS && (header.bit_count == 16 || header.bit_count == 32) {
        offset += 12;
    }
    if header.bit_count <= 8 {
        let colors = if header.colors_used != 0 {
            header.colors_used
        } else {
            1u32 << header.bit_count
        };
        offset += colors as usize * 4;
    }
    if offset > dib.len() {
        return Err(anyhow!("CF_DIB pixel offset outside buffer"));
    }
    Ok(offset)
}

fn dib_stride(width: u32, bit_count: u16) -> usize {
    ((width as usize * bit_count as usize + 31) / 32) * 4
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_i32(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}
