use crate::crypto::KeyManager;
use crate::db::{Database, NewItem};
use anyhow::{anyhow, Result};
use arboard::{Clipboard, ImageData};
use image::ImageFormat;
use image::GenericImageView;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::NSString;
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

//

#[cfg(target_os = "macos")]
pub fn poll_pasteboard_sync(app_handle: tauri::AppHandle, state: crate::state::AppState) -> Result<()> {
    use tauri::Manager;
    use cocoa::foundation::NSUInteger;
    use std::time::Duration;

    unsafe {
        let mut last: NSUInteger = 0;
        loop {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            let count: NSUInteger = msg_send![pb, changeCount];
            if count != last {
                last = count;
                if let Err(e) = handle_change(pb, state.db.clone(), state.crypto.clone()) {
                    eprintln!("pasteboard read error: {e:?}");
                } else {
                    let _ = app_handle.emit_all("items_updated", ());
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

#[cfg(target_os = "macos")]
fn handle_change(pb: id, db: Arc<Database>, crypto: Arc<KeyManager>) -> Result<()> {
    // 1) File URLs
    let file_paths = read_file_urls(pb);
    if !file_paths.is_empty() {
        for p in file_paths {
            let path = PathBuf::from(&p);
            let size = std::fs::metadata(&path).map(|m| m.len() as i64).unwrap_or(0);
            let sha = Database::compute_sha256(p.as_bytes());
            let item = NewItem {
                kind: "file".into(),
                size,
                sha256: sha,
                file_path: Some(p),
                content_blob: None,
                preview_blob: None,
                rtf_blob: None,
            };
            let _ = db.insert_item(item);
        }
        return Ok(());
    }

    // 2) RTF (optional)
    let rtf_data = read_rtf_data(pb);

    // 3) Text & Image via arboard
    let mut cb = Clipboard::new().ok();
    let mut captured = false;
    if let Some(ref mut c) = cb {
        if let Ok(text) = c.get_text() {
            captured = true;
            if crypto.is_unlocked() {
                let enc = crypto.encrypt(text.as_bytes())?;
                let sha = Database::compute_sha256(text.as_bytes());
                let item = NewItem {
                    kind: "text".into(),
                    size: text.len() as i64,
                    sha256: sha,
                    file_path: None,
                    content_blob: Some(enc),
                    preview_blob: None,
                    rtf_blob: rtf_data.as_ref().and_then(|d| crypto.encrypt(d).ok()),
                };
                let _ = db.insert_item(item);
                return Ok(());
            }
        }
        if let Ok(img) = c.get_image() {
            captured = true;
            let png = rgba_to_png(&img)?;
            if crypto.is_unlocked() {
                let enc = crypto.encrypt(&png)?;
                let sha = Database::compute_sha256(&png);
                let item = NewItem {
                    kind: "image".into(),
                    size: png.len() as i64,
                    sha256: sha,
                    file_path: None,
                    content_blob: Some(enc),
                    preview_blob: None, // lazy thumbnails in UI
                    rtf_blob: None,
                };
                let _ = db.insert_item(item);
                return Ok(());
            }
        }
    }

    if !captured {
        // Unknown types ignored
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn read_file_urls(pb: id) -> Vec<String> {
    unsafe {
        let items: id = msg_send![pb, pasteboardItems];
        if items == nil {
            return vec![];
        }
        let count: u64 = msg_send![items, count];
        if count == 0 {
            return vec![];
        }
        let ty: id = NSString::alloc(nil).init_str("public.file-url");
        let mut out = Vec::new();
        for i in 0..count {
            let item: id = msg_send![items, objectAtIndex: i];
            let s: id = msg_send![item, stringForType: ty];
            if s != nil {
                let cstr: *const std::os::raw::c_char = msg_send![s, UTF8String];
                if cstr.is_null() { continue; }
                let mut raw = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();
                raw = raw.trim().to_string();
                let mut pushed = false;
                if raw.starts_with("file:") {
                    if let Ok(u) = url::Url::parse(&raw) {
                        if let Ok(p) = u.to_file_path() {
                            out.push(p.to_string_lossy().to_string());
                            pushed = true;
                        }
                    }
                }
                if !pushed {
                    let tmp = raw
                        .trim_start_matches("file://localhost")
                        .trim_start_matches("file://");
                    let decoded = urlencoding::decode(tmp).unwrap_or_else(|_| tmp.into());
                    out.push(decoded.to_string());
                }
            }
        }
        out
    }
}

#[cfg(target_os = "macos")]
fn read_rtf_data(pb: id) -> Option<Vec<u8>> {
    unsafe {
        let items: id = msg_send![pb, pasteboardItems];
        if items == nil {
            return None;
        }
        let count: u64 = msg_send![items, count];
        if count == 0 {
            return None;
        }
        let ty: id = NSString::alloc(nil).init_str("public.rtf");
        for i in 0..count {
            let item: id = msg_send![items, objectAtIndex: i];
            let data: id = msg_send![item, dataForType: ty];
            if data != nil {
                let len: u64 = msg_send![data, length];
                let bytes: *const u8 = msg_send![data, bytes];
                if !bytes.is_null() && len > 0 {
                    let slice = std::slice::from_raw_parts(bytes, len as usize);
                    return Some(slice.to_vec());
                }
            }
        }
        None
    }
}

fn rgba_to_png(img: &ImageData) -> Result<Vec<u8>> {
    let (w, h) = (img.width as u32, img.height as u32);
    let buf = image::RgbaImage::from_raw(w, h, img.bytes.to_vec())
        .ok_or_else(|| anyhow!("bad rgba buffer"))?;
    let mut out = Vec::new();
    let img_dyn = image::DynamicImage::ImageRgba8(buf);
    img_dyn.write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)?;
    Ok(out)
}

pub fn copy_back(db: &Database, crypto: &KeyManager, id: i64) -> Result<()> {
    let (kind, content_blob, _preview_blob, rtf_blob, file_path) = db.get_item_raw(id)?;
    match kind.as_str() {
        "text" => {
            if let Some(ct) = content_blob {
                let pt = crypto.decrypt(&ct)?;
                let mut cb = Clipboard::new()?;
                cb.set_text(String::from_utf8_lossy(&pt).to_string())?;
            }
        }
        "image" => {
            if let Some(ct) = content_blob {
                let pt = crypto.decrypt(&ct)?; // PNG bytes
                let img = image::load_from_memory(&pt)?;
                let rgba = img.to_rgba8();
                let (w, h) = img.dimensions();
                let data = ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: std::borrow::Cow::Owned(rgba.into_raw()),
                };
                let mut cb = Clipboard::new()?;
                cb.set_image(data)?;
            }
        }
        "file" => {
            #[cfg(target_os = "macos")]
            {
                if let Some(path) = file_path {
                    unsafe {
                        let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
                        let _: () = msg_send![pb, clearContents];
                        let ns_path: id = NSString::alloc(nil).init_str(&path);
                        let url: id = msg_send![class!(NSURL), fileURLWithPath: ns_path];
                        let arr: id = msg_send![class!(NSArray), arrayWithObject: url];
                        let _: bool = msg_send![pb, writeObjects: arr];
                    }
                }
            }
        }
        _ => {}
    }

    // Optionally set RTF if available (macOS), alongside plain text
    #[cfg(target_os = "macos")]
    if let Some(rtf) = rtf_blob {
        if let Ok(pt) = crypto.decrypt(&rtf) {
            unsafe {
                let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
                let _: () = msg_send![pb, clearContents];
                let nsdata: id = msg_send![class!(NSData), dataWithBytes: pt.as_ptr() as *const _ length: pt.len() as u64];
                let item: id = msg_send![class!(NSPasteboardItem), new];
                let uti: id = NSString::alloc(nil).init_str("public.rtf");
                let _: bool = msg_send![item, setData: nsdata forType: uti];
                let arr: id = msg_send![class!(NSArray), arrayWithObject: item];
                let _: bool = msg_send![pb, writeObjects: arr];
            }
        }
    }
    Ok(())
}
