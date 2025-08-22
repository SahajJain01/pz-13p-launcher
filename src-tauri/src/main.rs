#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
  fs,
  io,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::Duration
};
use tauri::Manager;
use walkdir::WalkDir;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const APPID: &str = "108600"; // PZ

#[derive(Serialize)]
struct DetectResp { steam_root: String, workshop_path: String, mods_path: String }

fn user_mods_dir() -> PathBuf {
  let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:/"));
  home.join("Zomboid").join("mods")
}

fn steam_root_from_registry() -> Option<String> {
  if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
    if let Ok(sp): Result<String, _> = hkcu.get_value("SteamPath") { return Some(sp) }
  }
  None
}

fn parse_libraryfolders(steam_root: &str) -> Vec<PathBuf> {
  let mut libs = vec![PathBuf::from(steam_root).join("steamapps")];
  let vdf = libs[0].join("libraryfolders.vdf");
  if let Ok(txt) = fs::read_to_string(&vdf) {
    let re = Regex::new(r#"path"\s*"([^"]+)"#).unwrap();
    for cap in re.captures_iter(&txt) {
      let p = PathBuf::from(&cap[1]).join("steamapps");
      if p.exists() { libs.push(p) }
    }
  }
  libs
}

fn find_workshop_item(steam_root: &str, workshop_id: &str) -> Option<String> {
  for lib in parse_libraryfolders(steam_root) {
    let p = lib.join("workshop").join("content").join(APPID).join(workshop_id);
    if p.exists() { return Some(p.to_string_lossy().to_string()) }
  }
  None
}

#[tauri::command]
fn auto_detect(appid: String, workshop_id: String) -> DetectResp {
  let steam_root = steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
  let workshop_path = find_workshop_item(&steam_root, &workshop_id).unwrap_or_default();
  let mods_path = user_mods_dir().to_string_lossy().to_string();
  fs::create_dir_all(&mods_path).ok();
  DetectResp { steam_root, workshop_path, mods_path }
}

#[tauri::command]
fn open_workshop(workshop_id: String) -> Result<(), String> {
  let url = format!("steam://url/CommunityFilePage/{}", workshop_id);
  open::that(url).map_err(|e| e.to_string())
}

fn dir_size(path: &Path) -> u64 {
  let mut total = 0;
  for e in WalkDir::new(path).min_depth(0).into_iter().filter_map(|e| e.ok()) {
    if e.file_type().is_file() {
      if let Ok(md) = e.metadata() { total += md.len(); }
    }
  }
  total
}

#[tauri::command]
fn wait_for_download(workshop_path: String) -> Result<(), String> {
  let mods = Path::new(&workshop_path).join("mods");
  let mut last = 0; let mut stable = 0;
  for _ in 0..6000 { // ~100 minutes max
    if mods.exists() {
      let sz = dir_size(&mods);
      if sz == last { stable += 1 } else { stable = 0; last = sz; }
      if stable >= 5 { return Ok(()); }
    }
    thread::sleep(Duration::from_secs(1));
  }
  Err("Timeout waiting for download".into())
}

#[derive(Serialize, Deserialize, Default)]
struct LockState { links: Vec<String>, backups: Vec<(String, String)> }

fn is_reparse_point(p: &Path) -> bool {
  #[cfg(windows)] {
    use std::os::windows::fs::MetadataExt;
    if let Ok(md) = fs::metadata(p) { return (md.file_attributes() & 0x400) != 0; } // FILE_ATTRIBUTE_REPARSE_POINT
    false
  }
  #[cfg(not(windows))] { false }
}

fn mk_junction(link: &Path, target: &Path) -> io::Result<()> {
  // cmd /c mklink /J "link" "target"
  let status = Command::new("cmd")
    .args(["/c", "mklink", "/J", &format!("\"{}\"", link.display()), &format!("\"{}\"", target.display())])
    .status()?;
  if status.success() { Ok(()) } else { Err(io::Error::new(io::ErrorKind::Other, "mklink failed")) }
}

fn rmdir_link(p: &Path) -> io::Result<()> {
  let status = Command::new("cmd").args(["/c", "rmdir", &format!("\"{}\"", p.display())]).status()?;
  if status.success() { Ok(()) } else { Err(io::Error::new(io::ErrorKind::Other, "rmdir failed")) }
}

#[tauri::command]
fn link_all(workshop_path: String, mods_path: String) -> Result<serde_json::Value, String> {
  let src_root = Path::new(&workshop_path).join("mods");
  if !src_root.exists() { return Err("No 'mods' folder in workshop item".into()); }
  fs::create_dir_all(&mods_path).map_err(|e| e.to_string())?;
  let backup_root = Path::new(&mods_path).join("_pzpack_backups");
  fs::create_dir_all(&backup_root).ok();

  let mut state = LockState::default();
  for entry in fs::read_dir(&src_root).map_err(|e| e.to_string())? {
    let entry = entry.map_err(|e| e.to_string())?;
    if !entry.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
    let name = entry.file_name();
    let name = name.to_string_lossy();

    let dst = Path::new(&mods_path).join(&*name);
    if dst.exists() && !is_reparse_point(&dst) {
      let bak = backup_root.join(format!("{}_{}", name, chrono::Utc::now().format("%Y%m%d%H%M%S")));
      fs::rename(&dst, &bak).map_err(|e| e.to_string())?;
      state.backups.push((dst.to_string_lossy().to_string(), bak.to_string_lossy().to_string()));
    }
    if !dst.exists() {
      mk_junction(&dst, &entry.path()).map_err(|e| e.to_string())?;
      state.links.push(dst.to_string_lossy().to_string());
    }
  }
  let lock_path = Path::new(&mods_path).join(".pz-links.json");
  fs::write(&lock_path, serde_json::to_vec(&state).unwrap()).map_err(|e| e.to_string())?;
  Ok(serde_json::json!({"linked": state.links.len(), "backups": state.backups.len()}))
}

#[tauri::command]
fn cleanup(mods_path: String) -> Result<serde_json::Value, String> {
  let lock_path = Path::new(&mods_path).join(".pz-links.json");
  if !lock_path.exists() { return Ok(serde_json::json!({"removed":0,"restored":0})); }
  let bytes = fs::read(&lock_path).map_err(|e| e.to_string())?;
  let state: LockState = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
  let mut removed = 0; let mut restored = 0;

  for link in &state.links {
    let p = Path::new(link);
    if p.exists() { rmdir_link(p).map_err(|e| e.to_string())?; removed += 1; }
  }
  for (dst, bak) in &state.backups {
    let d = Path::new(dst); let b = Path::new(bak);
    if !d.exists() && b.exists() { fs::rename(b, d).map_err(|e| e.to_string())?; restored += 1; }
  }
  let _ = fs::remove_file(&lock_path);
  Ok(serde_json::json!({"removed": removed, "restored": restored}))
}

#[tauri::command]
fn play(appid: String) -> Result<(), String> {
  // Launch Steam -> PZ and wait for Steam child to exit is tricky; we just invoke and return.
  // Users will hit Cleanup manually, or you can press Clean Links after quitting.
  let url = format!("steam://run/{}", appid);
  open::that(url).map_err(|e| e.to_string())
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![auto_detect, open_workshop, wait_for_download, link_all, cleanup, play])
    .run(tauri::generate_context!())
    .expect("error while running tauri app");
}