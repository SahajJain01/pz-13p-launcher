#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::Serialize;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};
use regex::Regex;

use sysinfo::System;
use walkdir::WalkDir;

use std::{
  fs,
  io,
  path::{Path, PathBuf},
  process::Command,
  thread,
  time::Duration
};

const APPID: &str = "108600"; // PZ

#[derive(Serialize)]
struct DetectResp {
  steam_root: String,
  workshop_path: String,
  mods_path: String,
}

fn user_mods_dir() -> PathBuf {
  let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:/"));
  home.join("Zomboid").join("mods")
}

fn steam_root_from_registry() -> Option<String> {
  if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
    if let Ok(sp) = hkcu.get_value::<String, _>("SteamPath") { return Some(sp) }
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
    if p.exists() {
      let s = p.to_string_lossy().replace('/', "\\");
      return Some(s)
    }
  }
  None
}

#[tauri::command]
fn auto_detect(workshop_id: String) -> DetectResp {
  let steam_root = steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
  // Check if PZ is installed by looking for the app manifest
  let mut pz_installed = false;
  let mut workshop_path = String::new();
  for lib in parse_libraryfolders(&steam_root) {
    let manifest = lib.join("appmanifest_108600.acf");
    if manifest.exists() {
      pz_installed = true;
      // Also try to find the workshop path if possible
      if let Some(wp) = find_workshop_item(&steam_root, &workshop_id) {
        workshop_path = wp.replace('/', "\\");
      }
      break;
    }
  }
  let mods_path = user_mods_dir().to_string_lossy().to_string();
  if !pz_installed {
    // Not installed, don't launch Steam or open workshop
    return DetectResp { steam_root, workshop_path, mods_path };
  }
  // If the mod folder is not found, open the workshop page for the user to subscribe
  if workshop_path.is_empty() || !Path::new(&workshop_path).exists() {
    let url = format!("steam://url/CommunityFilePage/{}", workshop_id);
    let _ = open::that(url);
  }
  fs::create_dir_all(&mods_path).ok();
  DetectResp { steam_root, workshop_path, mods_path }
}

#[tauri::command]
fn open_workshop(workshop_id: String) -> Result<(), String> {
  let url = format!("steam://url/CommunityFilePage/{}", workshop_id);
  open::that(url).map_err(|e| e.to_string())
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
  // Use mklink /J for directory junctions on Windows. Always use backslashes and quote the paths.
  // Normalize both paths to absolute Windows paths with backslashes and quote them
  // Always use absolute, all-backslash Windows paths for mklink, even if they do not exist
  let link_abs = if link.is_absolute() {
    link.to_path_buf()
  } else {
    std::env::current_dir().unwrap().join(link)
  };
  let target_abs = if target.is_absolute() {
    target.to_path_buf()
  } else {
    std::env::current_dir().unwrap().join(target)
  };
  let link_str = format!("\"{}\"", link_abs.to_string_lossy().replace('/', "\\"));
  let target_str = format!("\"{}\"", target_abs.to_string_lossy().replace('/', "\\"));
  // Use PowerShell's New-Item cmdlet to create a directory junction
  let cmdline = format!(
    "powershell.exe -Command \"New-Item -ItemType Junction -Path {} -Target {}\"",
    link_str, target_str
  );
  let status = Command::new("powershell.exe")
    .args(["-Command", &format!(
      "New-Item -ItemType Junction -Path {} -Target {}",
      link_str, target_str
    )])
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::new(io::ErrorKind::Other, format!("PowerShell junction failed. Command: {}", cmdline)))
  }
}

fn rmdir_link(p: &Path) -> io::Result<()> {
  // Remove directory junction (symlink) on Windows
  let status = Command::new("cmd").args(["/C", "rmdir", &p.display().to_string()]).status()?;
  if status.success() { Ok(()) } else { Err(io::Error::new(io::ErrorKind::Other, "rmdir failed")) }
}

#[tauri::command]
fn link_all(workshop_path: String, mods_path: String) -> Result<serde_json::Value, String> {
  // Link the user's mods folder directly to the Zomboid folder inside the pseudo mod
  let target = Path::new(&workshop_path).join("mods").join("13thPandemic").join("Zomboid").join("mods");
  if !target.exists() {
    // Try to create the target directory if it does not exist
    if let Err(e) = std::fs::create_dir_all(&target) {
      return Err(format!("Failed to create target directory {}: {}", target.display(), e));
    }
  }
  // Remove the user's mods folder if it is a junction, or back it up if it is a real directory
  let mods_path_p = Path::new(&mods_path);
  let mut state = LockState::default();
  let backup_root = mods_path_p.parent().unwrap_or_else(|| Path::new("C:/")).join("_pzpack_backups");
  fs::create_dir_all(&backup_root).ok();
  if mods_path_p.exists() {
    if is_reparse_point(&mods_path_p) {
      // Remove the junction (do not back it up)
      rmdir_link(&mods_path_p).map_err(|e| format!("Failed to remove junction {}: {}", mods_path_p.display(), e))?;
    } else {
      // Back up real directory
      let bak = backup_root.join(format!("mods_{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));
      fs::rename(&mods_path_p, &bak).map_err(|e| e.to_string())?;
      state.backups.push((mods_path_p.to_string_lossy().to_string(), bak.to_string_lossy().to_string()));
    }
  }
  if !mods_path_p.exists() {
    mk_junction(&mods_path_p, &target).map_err(|e| format!("Failed to create symlink: {} -> {}: {}", mods_path_p.display(), target.display(), e))?;
    state.links.push(mods_path_p.to_string_lossy().to_string());
  }
  let lock_path = mods_path_p.parent().unwrap_or_else(|| Path::new("C:/")).join(".pz-links.json");
  fs::write(&lock_path, serde_json::to_vec(&state).unwrap()).map_err(|e| e.to_string())?;
  Ok(serde_json::json!({"linked": state.links.len(), "backups": state.backups.len()}))
}

#[tauri::command]
fn cleanup(mods_path: String) -> Result<serde_json::Value, String> {
  let mods_path_p = Path::new(&mods_path);
  let lock_path = mods_path_p.parent().unwrap_or_else(|| Path::new("C:/")).join(".pz-links.json");
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

const SERVER_IP: &str = "pz.13thpandemic.net";
const SERVER_PORT: u16 = 16261;

#[tauri::command]
fn play(appid: String) -> Result<(), String> {
  // Ensure Steam is running before launching PZ
  let steam_root = steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
  let mut sys = System::new_all();
  sys.refresh_processes();
  let steam_running = sys.processes().values().any(|p| p.name().eq_ignore_ascii_case("steam.exe"));
  if !steam_running {
    let steam_exe = Path::new(&steam_root).join("steam.exe");
    let _ = Command::new(steam_exe).spawn();
    // Give Steam a few seconds to start
    thread::sleep(Duration::from_secs(3));
  }
  // Launch Steam -> PZ and auto-connect to the server
  let url = format!(
    "steam://run/{}//-connect={} -port={}",
    appid, SERVER_IP, SERVER_PORT
  );
  open::that(&url).map_err(|e| e.to_string())?;

  // Wait for Project Zomboid process to exit
  // The process name is "ProjectZomboid64.exe" (for 64-bit)
  let pz_proc_name = "ProjectZomboid64.exe";
  let mut found = false;
  for _ in 0..10 {
    sys.refresh_processes();
    if sys.processes().values().any(|p| p.name().eq_ignore_ascii_case(pz_proc_name)) {
      found = true;
      break;
    }
    thread::sleep(Duration::from_secs(1));
  }
  if found {
    // Wait until the process is gone
    loop {
      sys.refresh_processes();
      if !sys.processes().values().any(|p| p.name().eq_ignore_ascii_case(pz_proc_name)) {
        break;
      }
      thread::sleep(Duration::from_secs(2));
    }
  }
  Ok(())
}

fn main() {
  // This launcher helps Project Zomboid private server users quickly link a large modpack from a single Steam Workshop pseudo mod.
  // 1. User subscribes to the pseudo mod (manually or via launcher).

  // 2. On Play, launcher symlinks all submods from the pseudo mod's workshop folder into the user's mods folder.
  // 3. Launches the game.
  // 4. On exit or cleanup, removes the symlinks and restores any backups.
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![auto_detect, open_workshop, link_all, cleanup, play])

    .run(tauri::generate_context!())
    .expect("error while running tauri app");
}