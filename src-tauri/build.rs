use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
  // Ensure a valid PNG icons from cliper_icon.webp if available
  let icons_dir = Path::new("icons");
  let icon_path = icons_dir.join("icon.png");
  let tray_path = icons_dir.join("tray.png");
  if let Err(e) = fs::create_dir_all(icons_dir) { eprintln!("warn: icons dir: {e}"); }

  if let Some(src) = find_clip_icon() {
    // Generate multiple icon sizes for bundler robustness
    let sizes = [16u32, 32, 64, 128, 256, 512, 1024];
    for s in sizes.iter() {
      let p = icons_dir.join(format!("icon_{}.png", s));
      if let Err(e) = convert_webp_to_png(&src, &p, *s) { eprintln!("warn: icon convert {}: {e}", s); }
    }
    // Keep a generic icon.png as well (use 512)
    let _ = convert_webp_to_png(&src, &icon_path, 512);
    // Generate .icns for app bundle on macOS
    let icon_1024 = icons_dir.join("icon_1024.png");
    generate_icns_from_png(&icon_1024, &icons_dir.join("icon.icns"));
    // Tray icon small
    let _ = convert_webp_to_png(&src, &tray_path, 22);
  } else {
    if let Err(e) = generate_rgba_icon(&icon_path) { eprintln!("warn: generate icon: {e}"); }
    let _ = fs::copy(&icon_path, &tray_path);
  }
  tauri_build::build()
}

fn generate_rgba_icon(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
  let mut img = image::RgbaImage::new(128, 128);
  // draw a simple dot
  for y in 0..128u32 {
    for x in 0..128u32 {
      let a = 180u8;
      let val = if (x as i32 - 64).abs() + (y as i32 - 64).abs() < 8 { 240 } else { 32 };
      img.put_pixel(x, y, image::Rgba([val, val, val, a]));
    }
  }
  img.save(path)?;
  Ok(())
}

fn find_clip_icon() -> Option<PathBuf> {
  let candidates = [
    PathBuf::from("../cliper_icon.webp"),
    PathBuf::from("cliper_icon.webp"),
    PathBuf::from("../assets/cliper_icon.webp"),
  ];
  for p in candidates.iter() { if p.exists() { return Some(p.clone()); } }
  None
}

fn convert_webp_to_png(src: &Path, dst: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
  let dynimg = image::open(src)?;
  let resized = dynimg.resize(size, size, image::imageops::FilterType::Lanczos3);
  let rgba = resized.to_rgba8();
  image::DynamicImage::ImageRgba8(rgba).save(dst)?;
  Ok(())
}

#[cfg(target_os = "macos")]
fn generate_icns_from_png(png_1024: &Path, out_icns: &Path) {
  // Prepare iconset directory
  let iconset_dir = out_icns.with_file_name("Cliper.iconset");
  let _ = fs::create_dir_all(&iconset_dir);
  let sizes = [
    (16, "icon_16x16.png"),
    (32, "icon_16x16@2x.png"),
    (32, "icon_32x32.png"),
    (64, "icon_32x32@2x.png"),
    (128, "icon_128x128.png"),
    (256, "icon_128x128@2x.png"),
    (256, "icon_256x256.png"),
    (512, "icon_256x256@2x.png"),
    (512, "icon_512x512.png"),
  ];
  for (sz, name) in sizes.iter() {
    let out = iconset_dir.join(name);
    let _ = Command::new("sips")
      .args(["-z", &sz.to_string(), &sz.to_string(), png_1024.to_string_lossy().as_ref(), "--out", out.to_string_lossy().as_ref()])
      .output();
  }
  // 1024 @2x
  let _ = fs::copy(png_1024, iconset_dir.join("icon_512x512@2x.png"));
  let _ = Command::new("iconutil")
    .args(["-c", "icns", iconset_dir.to_string_lossy().as_ref(), "-o", out_icns.to_string_lossy().as_ref()])
    .output();
}

#[cfg(not(target_os = "macos"))]
fn generate_icns_from_png(_png_1024: &Path, _out_icns: &Path) { }
