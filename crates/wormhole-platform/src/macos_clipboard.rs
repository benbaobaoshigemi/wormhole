use anyhow::{anyhow, Result};
use cocoa::{
    base::{id, nil},
    foundation::NSString,
};
use objc::{class, msg_send, sel, sel_impl};
use std::{ffi::CStr, os::raw::c_void};
use wormhole_core::ClipboardPort;

const NSPNG_FILE_TYPE: u64 = 4;

pub struct SystemClipboard;

impl SystemClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl ClipboardPort for SystemClipboard {
    fn read_text(&mut self) -> Result<Option<String>> {
        unsafe {
            let pb = pasteboard();
            let ty = nsstring("public.utf8-plain-text");
            let value: id = msg_send![pb, stringForType: ty];
            if value == nil {
                return Ok(None);
            }
            let c: *const i8 = msg_send![value, UTF8String];
            if c.is_null() {
                return Ok(None);
            }
            Ok(Some(CStr::from_ptr(c).to_string_lossy().to_string()))
        }
    }

    fn write_text(&mut self, text: &str) -> Result<()> {
        unsafe {
            let pb = pasteboard();
            let _: () = msg_send![pb, clearContents];
            let value = nsstring(text);
            let ty = nsstring("public.utf8-plain-text");
            let ok: bool = msg_send![pb, setString: value forType: ty];
            if ok {
                Ok(())
            } else {
                Err(anyhow!("NSPasteboard setString failed"))
            }
        }
    }

    fn read_png(&mut self) -> Result<Option<Vec<u8>>> {
        unsafe {
            let pb = pasteboard();
            let png_ty = nsstring("public.png");
            let data: id = msg_send![pb, dataForType: png_ty];
            if data != nil {
                return nsdata_to_vec(data).map(Some);
            }
            let tiff_ty = nsstring("public.tiff");
            let tiff: id = msg_send![pb, dataForType: tiff_ty];
            if tiff == nil {
                return Ok(None);
            }
            let png = tiff_nsdata_to_png(tiff)?;
            Ok(Some(png))
        }
    }

    fn write_png(&mut self, png: &[u8]) -> Result<()> {
        unsafe {
            let pb = pasteboard();
            let _: () = msg_send![pb, clearContents];
            let data = nsdata_from_bytes(png);
            let ty = nsstring("public.png");
            let ok: bool = msg_send![pb, setData: data forType: ty];
            if ok {
                Ok(())
            } else {
                Err(anyhow!("NSPasteboard setData public.png failed"))
            }
        }
    }
}

unsafe fn pasteboard() -> id {
    msg_send![class!(NSPasteboard), generalPasteboard]
}

unsafe fn nsstring(value: &str) -> id {
    NSString::alloc(nil).init_str(value)
}

unsafe fn nsdata_from_bytes(bytes: &[u8]) -> id {
    msg_send![
        class!(NSData),
        dataWithBytes: bytes.as_ptr() as *const c_void
        length: bytes.len()
    ]
}

unsafe fn nsdata_to_vec(data: id) -> Result<Vec<u8>> {
    let len: usize = msg_send![data, length];
    let ptr: *const u8 = msg_send![data, bytes];
    if ptr.is_null() {
        return Err(anyhow!("NSData bytes is null"));
    }
    Ok(std::slice::from_raw_parts(ptr, len).to_vec())
}

unsafe fn tiff_nsdata_to_png(tiff: id) -> Result<Vec<u8>> {
    let image: id = msg_send![class!(NSImage), alloc];
    let image: id = msg_send![image, initWithData: tiff];
    if image == nil {
        return Err(anyhow!(
            "NSImage initWithData failed for TIFF pasteboard data"
        ));
    }
    let tiff_repr: id = msg_send![image, TIFFRepresentation];
    if tiff_repr == nil {
        return Err(anyhow!("NSImage TIFFRepresentation failed"));
    }
    let rep: id = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff_repr];
    if rep == nil {
        return Err(anyhow!("NSBitmapImageRep imageRepWithData failed"));
    }
    let props: id = msg_send![class!(NSDictionary), dictionary];
    let png: id = msg_send![rep, representationUsingType: NSPNG_FILE_TYPE properties: props];
    if png == nil {
        return Err(anyhow!("NSBitmapImageRep PNG representation failed"));
    }
    nsdata_to_vec(png)
}
