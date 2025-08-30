#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::{Serialize, Deserialize};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};
use regex::Regex;

use sysinfo::System;

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
  let status = Command::new("powershell.exe")
    .args(["-Command", &format!(
      "New-Item -ItemType Junction -Path {} -Target {}",
      link_str, target_str
    )])
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::new(
      io::ErrorKind::Other,
      format!("PowerShell junction failed: {} -> {}", link_str, target_str),
    ))
  }
}

fn rmdir_link(p: &Path) -> io::Result<()> {
  // Remove directory junction (symlink) on Windows
  let status = Command::new("cmd").args(["/C", "rmdir", &p.display().to_string()]).status()?;
  if status.success() { Ok(()) } else { Err(io::Error::new(io::ErrorKind::Other, "rmdir failed")) }
}

fn ps_escape_single(s: &str) -> String { s.replace('\'', "''") }

fn query_junction_target(p: &Path) -> Option<PathBuf> {
  // Query junction target using PowerShell's Get-Item .Target
  let p_str = p.to_string_lossy().replace('/', "\\");
  let script = format!("(Get-Item -LiteralPath '{}' -Force).Target", ps_escape_single(&p_str));
  if let Ok(out) = Command::new("powershell.exe").args(["-NoProfile", "-Command", &script]).output() {
    if out.status.success() {
      let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
      if !s.is_empty() { return Some(PathBuf::from(s)); }
    }
  }
  None
}

fn move_dir(src: &Path, dst: &Path) -> io::Result<()> {
  if let Some(parent) = dst.parent() { let _ = fs::create_dir_all(parent); }
  // Try fast rename first (same volume)
  match fs::rename(src, dst) {
    Ok(()) => return Ok(()),
    Err(_e) => {
      // Fallback to robocopy /MOVE for cross-volume move; treat exit codes < 8 as success
      let status = Command::new("robocopy")
        .arg(src)
        .arg(dst)
        .args(["/MOVE", "/E", "/R:1", "/W:1"])
        .status();
      match status {
        Ok(s) => {
          // robocopy returns codes: <8 success; >=8 failure
          if let Some(code) = s.code() {
            if code < 8 { return Ok(()); }
          }
          Err(io::Error::new(io::ErrorKind::Other, format!("robocopy failed with status {:?}", s.code())))
        }
        Err(e2) => Err(io::Error::new(io::ErrorKind::Other, format!("robocopy exec failed: {}", e2)))
      }
    }
  }
}

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
  // legacy: whole workshop folder move (unused now but kept for compatibility)
  workshop_locations: std::collections::HashMap<String, String>,
  // new: moved mods folder absolute path
  mods_locations: std::collections::HashMap<String, String>,
}

fn config_file_path() -> PathBuf {
  let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("C:/"));
  base.join("pz-13p-launcher").join("config.json")
}

fn load_config() -> AppConfig {
  let p = config_file_path();
  if let Ok(bytes) = fs::read(&p) {
    if let Ok(cfg) = serde_json::from_slice::<AppConfig>(&bytes) { return cfg; }
  }
  AppConfig::default()
}

fn save_config(cfg: &AppConfig) -> io::Result<()> {
  let p = config_file_path();
  if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent); }
  let data = serde_json::to_vec_pretty(cfg)?;
  fs::write(p, data)
}

#[tauri::command]
fn move_workshop(workshop_path: String, workshop_id: String, dest_dir: String) -> Result<serde_json::Value, String> {
  // Move only the internal Zomboid Mods folder: <workshop>/mods/13thPandemic/Zomboid/Mods
  if workshop_path.is_empty() { return Err("Workshop path is empty".into()); }
  if dest_dir.is_empty() { return Err("Destination directory is empty".into()); }

  let workshop = Path::new(&workshop_path);
  if !workshop.exists() { return Err(format!("Workshop path does not exist: {}", workshop.display())); }
  let zroot = workshop_zomboid_root(workshop);
  let mods_link = zroot.join("Mods");
  let dest_parent = Path::new(&dest_dir);
  if !dest_parent.exists() { fs::create_dir_all(dest_parent).map_err(|e| format!("Failed to create destination: {}", e))?; }

  // Determine new mods target path; if the selected path already ends with 'Mods', use it, else create a 'Mods' under it
  let new_mods_path = if dest_parent.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("mods")).unwrap_or(false) {
    dest_parent.to_path_buf()
  } else {
    dest_parent.join("Mods")
  };

  // Determine current real mods folder (handle junction)
  let mut current_real = mods_link.clone();
  let was_junction = is_reparse_point(&mods_link);
  if was_junction {
    if let Some(t) = query_junction_target(&mods_link) { current_real = t; }
  }

  // If destination equals current, nothing to move
  let need_move = new_mods_path.to_string_lossy().to_lowercase() != current_real.to_string_lossy().to_lowercase();
  if need_move {
    if new_mods_path.exists() {
      return Err(format!("Destination already exists: {}", new_mods_path.display()));
    }
    move_dir(&current_real, &new_mods_path).map_err(|e| format!("Failed to move mods folder: {}", e))?;
  }

  // Ensure junction at Zomboid/Mods points to new_mods_path
  if mods_link.exists() && is_reparse_point(&mods_link) {
    rmdir_link(&mods_link).map_err(|e| format!("Failed to remove existing mods junction: {}", e))?;
  } else if mods_link.exists() {
    // Remove or back up real directory if still present
    if fs::read_dir(&mods_link).map(|mut it| it.next().is_none()).unwrap_or(false) {
      fs::remove_dir(&mods_link).ok();
    } else {
      let backup = workshop.join(format!("mods_bak_{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));
      fs::rename(&mods_link, &backup).map_err(|e| format!("Failed to relocate original mods dir: {}", e))?;
    }
  }
  mk_junction(&mods_link, &new_mods_path).map_err(|e| format!("Failed to create mods junction: {}", e))?;

  // Persist new mods location for this workshop id
  let mut cfg = load_config();
  cfg.mods_locations.insert(workshop_id.clone(), new_mods_path.to_string_lossy().to_string());
  save_config(&cfg).map_err(|e| format!("Failed to save config: {}", e))?;

  Ok(serde_json::json!({
    "new_path": new_mods_path.to_string_lossy().to_string(),
    "updated_link": true,
    "was_junction": was_junction
  }))
}

#[tauri::command]
fn restore_workshop(workshop_path: String, workshop_id: String) -> Result<serde_json::Value, String> {
  // Restore the internal Zomboid Mods folder back to <workshop>/mods/13thPandemic/Zomboid/Mods and remove the junction
  if workshop_path.is_empty() { return Err("Workshop path is empty".into()); }

  let workshop = Path::new(&workshop_path);
  let zroot = workshop_zomboid_root(workshop);
  let mods_link = zroot.join("Mods");
  let mut real_mods = mods_link.clone();
  let is_junc = is_reparse_point(&mods_link);
  if is_junc {
    real_mods = query_junction_target(&mods_link).ok_or_else(|| "Could not resolve mods junction target".to_string())?;
    rmdir_link(&mods_link).map_err(|e| format!("Failed to remove mods junction: {}", e))?;
  } else {
    // if config points to moved location, use it
    if let Some(p) = load_config().mods_locations.get(&workshop_id) {
      real_mods = PathBuf::from(p);
    }
  }

  if !real_mods.exists() {
    return Err(format!("Real mods folder not found: {}", real_mods.display()));
  }
  if mods_link.exists() {
    return Err(format!("Mods path already exists at workshop: {}", mods_link.display()));
  }

  move_dir(&real_mods, &mods_link).map_err(|e| format!("Failed to move mods back: {}", e))?;

  // Clear persisted location
  let mut cfg = load_config();
  cfg.mods_locations.remove(&workshop_id);
  save_config(&cfg).map_err(|e| format!("Failed to save config: {}", e))?;

  Ok(serde_json::json!({ "restored": true, "path": mods_link.to_string_lossy().to_string() }))
}
#[tauri::command]
fn resolve_workshop_mods(workshop_path: String) -> Result<String, String> {
  // Returns the real path of <workshop>/mods/13thPandemic/Zomboid/Mods if it's a junction, else the path itself
  let zroot = workshop_zomboid_root(Path::new(&workshop_path));
  let mods_link = zroot.join("Mods");
  if is_reparse_point(&mods_link) {
    if let Some(t) = query_junction_target(&mods_link) {
      return Ok(t.to_string_lossy().to_string());
    }
  }
  Ok(mods_link.to_string_lossy().to_string())
}

#[tauri::command]
fn resolve_mods(mods_path: String) -> Result<String, String> {
  let p = Path::new(&mods_path);
  if is_reparse_point(&p) {
    if let Some(t) = query_junction_target(&p) {
      return Ok(t.to_string_lossy().to_string());
    }
  }
  Ok(mods_path)
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
  if path.is_empty() { return Err("Empty path".into()); }
  open::that(path).map_err(|e| e.to_string())
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
  let data = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
  fs::write(&lock_path, data).map_err(|e| e.to_string())?;
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

fn workshop_zomboid_root(real_workshop_path: &Path) -> PathBuf {
  real_workshop_path
    .join("mods")
    .join("13thPandemic")
    .join("Zomboid")
}

#[tauri::command]
fn play(appid: String, _workshop_id: String, workshop_path: String) -> Result<(), String> {
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
  // Always point cachedir to the workshop Zomboid folder; Mods may be a junction to another drive
  let cachedir = workshop_zomboid_root(Path::new(&workshop_path));
  // Ensure the cachedir exists
  fs::create_dir_all(&cachedir).map_err(|e| format!("Failed to create cachedir {}: {}", cachedir.display(), e))?;

  // Launch Steam -> PZ with -cachedir and auto-connect using -applaunch
  let steam_exe = Path::new(&steam_root).join("steam.exe");
  let cachedir_arg = format!("-cachedir={}", cachedir.to_string_lossy().replace('/', "\\"));
  let _ = Command::new(steam_exe)
    .arg("-applaunch")
    .arg(appid)
    .arg(cachedir_arg)
    .arg(format!("-connect={}", SERVER_IP))
    .arg(format!("-port={}", SERVER_PORT))
    .spawn()
    .map_err(|e| format!("Failed to launch Steam/PZ: {}", e))?;

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
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![auto_detect, open_workshop, link_all, cleanup, play, move_workshop, resolve_workshop_mods, resolve_mods, restore_workshop, open_path])

    .run(tauri::generate_context!())
    .expect("error while running tauri app");
}
