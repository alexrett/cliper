use std::fs;
use std::path::{Path, PathBuf};

fn main() {
  // Ensure a valid PNG icons from cliper_icon.webp if available
  let icons_dir = Path::new("icons");
  let icon_path = icons_dir.join("icon.png");
  let tray_path = icons_dir.join("tray.png");
  if let Err(e) = fs::create_dir_all(icons_dir) { eprintln!("warn: icons dir: {e}"); }

  if let Some(src) = find_clip_icon() {
    if let Err(e) = convert_webp_to_png(&src, &icon_path, 512) { eprintln!("warn: icon convert: {e}"); }
    if let Err(e) = convert_webp_to_png(&src, &tray_path, 22) { eprintln!("warn: tray convert: {e}"); }
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
