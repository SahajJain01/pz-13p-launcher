#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use regex::Regex;
use serde::Serialize;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

use sysinfo::System;
use tauri::Emitter;

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

const APPID: &str = "108600"; // PZ
const SERVER_IP: &str = "pz.13thpandemic.net";
const SERVER_PORT: u16 = 16261;

#[derive(Serialize)]
struct DetectResp {
    steam_root: String,
    workshop_path: String,
}

fn steam_root_from_registry() -> Option<String> {
    if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(sp) = hkcu.get_value::<String, _>("SteamPath") {
            return Some(sp);
        }
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
            if p.exists() {
                libs.push(p)
            }
        }
    }
    libs
}

fn find_workshop_item(steam_root: &str, workshop_id: &str) -> Option<String> {
    for lib in parse_libraryfolders(steam_root) {
        let p = lib
            .join("workshop")
            .join("content")
            .join(APPID)
            .join(workshop_id);
        if p.exists() {
            let s = p.to_string_lossy().replace('/', "\\");
            return Some(s);
        }
    }
    None
}

#[tauri::command]
fn auto_detect(workshop_id: String) -> DetectResp {
    let steam_root =
        steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
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
    if !pz_installed {
        // Not installed, don't launch Steam or open workshop
        return DetectResp {
            steam_root,
            workshop_path,
        };
    }
    // If the mod folder is not found, open the workshop page for the user to subscribe
    if workshop_path.is_empty() || !Path::new(&workshop_path).exists() {
        let url = format!("steam://url/CommunityFilePage/{}", workshop_id);
        let _ = open::that(url);
    }
    DetectResp {
        steam_root,
        workshop_path,
    }
}

#[tauri::command]
fn open_workshop(workshop_id: String) -> Result<(), String> {
    let url = format!("steam://url/CommunityFilePage/{}", workshop_id);
    open::that(url).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    if path.is_empty() {
        return Err("Empty path".into());
    }
    open::that(path).map_err(|e| e.to_string())
}

fn workshop_zomboid_root(real_workshop_path: &Path) -> PathBuf {
    real_workshop_path
        .join("mods")
        .join("13thPandemic")
        .join("Zomboid")
}

fn pz_install_dir(steam_root: &str) -> Option<PathBuf> {
    for lib in parse_libraryfolders(steam_root) {
        let p = lib.join("common").join("ProjectZomboid");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn list_files_recursive(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for ent in fs::read_dir(&dir)? {
            let ent = ent?;
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    Ok(files)
}

fn files_already_applied(src_root: &Path, dst_root: &Path) -> bool {
    if !dst_root.exists() {
        return false;
    }
    let Ok(src_files) = list_files_recursive(src_root) else {
        return false;
    };
    if src_files.is_empty() {
        return false;
    }
    for s in src_files {
        let rel = match s.strip_prefix(src_root) {
            Ok(r) => r,
            Err(_) => return false,
        };
        let d = dst_root.join(rel);
        let sm = match fs::metadata(&s) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let dm = match fs::metadata(&d) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if sm.len() != dm.len() {
            return false;
        }
    }
    true
}

fn copy_dir_replace(src_root: &Path, dst_root: &Path) -> io::Result<(u64, u64)> {
    let mut copied: u64 = 0;
    let mut replaced: u64 = 0;
    for s in list_files_recursive(src_root)? {
        let rel = s.strip_prefix(src_root).unwrap();
        let d = dst_root.join(rel);
        if let Some(parent) = d.parent() {
            fs::create_dir_all(parent)?;
        }
        if d.exists() {
            fs::copy(&s, &d)?;
            replaced += 1;
        } else {
            fs::copy(&s, &d)?;
            copied += 1;
        }
    }
    Ok((copied, replaced))
}

#[tauri::command]
fn resolve_game_root() -> Result<String, String> {
    let steam_root =
        steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
    let p = pz_install_dir(&steam_root)
        .ok_or_else(|| "Project Zomboid install not found".to_string())?;
    Ok(p.to_string_lossy().to_string())
}

#[tauri::command]
fn apply_optimizations(workshop_path: String) -> Result<serde_json::Value, String> {
    if workshop_path.is_empty() {
        return Err("Workshop path is empty".into());
    }
    let steam_root =
        steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
    // Source: <workshop>\mods\13thPandemic\ProjectZomboid
    let src = Path::new(&workshop_path)
        .join("mods")
        .join("13thPandemic")
        .join("ProjectZomboid");
    if !src.exists() {
        return Err(format!("Optimizations folder not found: {}", src.display()));
    }
    let dest = pz_install_dir(&steam_root)
        .ok_or_else(|| "Could not locate ProjectZomboid install directory".to_string())?;

    if files_already_applied(&src, &dest) {
        return Ok(serde_json::json!({
          "already": true,
          "applied": false,
          "source": src.to_string_lossy().to_string(),
          "dest": dest.to_string_lossy().to_string()
        }));
    }

    let (copied, replaced) = copy_dir_replace(&src, &dest).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
      "already": false,
      "applied": true,
      "copied": copied,
      "replaced": replaced,
      "source": src.to_string_lossy().to_string(),
      "dest": dest.to_string_lossy().to_string()
    }))
}

#[tauri::command]
fn play(
    app_handle: tauri::AppHandle,
    appid: String,
    _workshop_id: String,
    workshop_path: String,
) -> Result<(), String> {
    if workshop_path.is_empty() {
        return Err("Workshop path is empty".into());
    }
    // Ensure Steam is running before launching PZ
    let steam_root =
        steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
    let mut sys = System::new_all();
    sys.refresh_processes();
    let steam_running = sys
        .processes()
        .values()
        .any(|p| p.name().eq_ignore_ascii_case("steam.exe"));
    if !steam_running {
        let steam_exe = Path::new(&steam_root).join("steam.exe");
        let _ = Command::new(&steam_exe).spawn();
        // Give Steam a few seconds to start
        thread::sleep(Duration::from_secs(3));
    }
    // Always point cachedir to the workshop Zomboid folder; Mods may be a junction to another drive
    let cachedir = workshop_zomboid_root(Path::new(&workshop_path));
    // Ensure the cachedir exists
    fs::create_dir_all(&cachedir)
        .map_err(|e| format!("Failed to create cachedir {}: {}", cachedir.display(), e))?;
    let cachedir_windows = cachedir.to_string_lossy().replace('/', "\\");

    // Launch Steam -> PZ with -cachedir and auto-connect using -applaunch
    let steam_exe = Path::new(&steam_root).join("steam.exe");
    let cachedir_arg = format!("-cachedir={}", cachedir_windows);
    Command::new(&steam_exe)
        .arg("-applaunch")
        .arg(appid)
        .arg(&cachedir_arg)
        .arg(format!("-connect={}", SERVER_IP))
        .arg(format!("-port={}", SERVER_PORT))
        .spawn()
        .map_err(|e| format!("Failed to launch Steam/PZ: {}", e))?;

    let launch_payload = serde_json::json!({ "cachedir": cachedir_windows.clone() });
    let _ = app_handle.emit("pz-session-launched", launch_payload);

    let handle_for_exit = app_handle.clone();
    let cachedir_for_exit = cachedir_windows.clone();
    thread::spawn(move || {
        let mut watcher = System::new_all();
        let proc_name = "ProjectZomboid64.exe";
        let mut found = false;
        for _ in 0..10 {
            watcher.refresh_processes();
            if watcher
                .processes()
                .values()
                .any(|p| p.name().eq_ignore_ascii_case(proc_name))
            {
                found = true;
                break;
            }
            thread::sleep(Duration::from_secs(1));
        }
        if found {
            loop {
                watcher.refresh_processes();
                if !watcher
                    .processes()
                    .values()
                    .any(|p| p.name().eq_ignore_ascii_case(proc_name))
                {
                    break;
                }
                thread::sleep(Duration::from_secs(2));
            }
        }
        let payload = serde_json::json!({
            "found": found,
            "cachedir": cachedir_for_exit,
        });
        let _ = handle_for_exit.emit("pz-session-ended", payload);
    });

    Ok(())
}

fn main() {
    // The launcher detects Steam/workshop paths, starts Project Zomboid with the modpack cachedir, and offers optional optimizations.
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            auto_detect,
            open_workshop,
            play,
            open_path,
            apply_optimizations,
            resolve_game_root
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
