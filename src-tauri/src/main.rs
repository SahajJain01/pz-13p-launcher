#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

use sysinfo::System;
use tauri::Emitter;

use std::{
    fs,
    io::{self, Read, Write},
    net::ToSocketAddrs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

const APPID: &str = "108600"; // PZ
const SERVER_IP: &str = "13thpandemic.mywire.org";
const SERVER_PORT: u16 = 16261;

#[derive(Serialize)]
struct DetectResp {
    steam_root: String,
    workshop_path: String,
}

#[derive(Serialize)]
struct ServerStatus {
    ip: String,
    ping_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ManifestEntry {
    path: String,
    size: u64,
    hash: String,
}

#[derive(Serialize, Deserialize)]
struct OptimizationManifest {
    entries: Vec<ManifestEntry>,
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

fn ping_host(host: &str) -> Option<u64> {
    let output = Command::new("ping")
        .arg("-n")
        .arg("1")
        .arg("-w")
        .arg("1000")
        .arg(host)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let re = Regex::new(r"time[=<]\s*(\d+)\s*ms").ok()?;
    let caps = re.captures(&stdout)?;
    let value = caps.get(1)?.as_str().parse::<u64>().ok()?;
    Some(value)
}

#[tauri::command]
fn get_server_status(host: String) -> Result<ServerStatus, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Host is empty".into());
    }
    let mut addrs = (host, 0)
        .to_socket_addrs()
        .map_err(|e| format!("Failed to resolve {}: {}", host, e))?;
    let ip = addrs
        .next()
        .ok_or_else(|| format!("No IP address found for {}", host))?
        .ip()
        .to_string();
    let ping_ms = ping_host(host);
    Ok(ServerStatus { ip, ping_ms })
}

fn launcher_root(real_workshop_path: &Path) -> PathBuf {
    real_workshop_path
        .join("mods")
        .join("13thPandemic")
        .join("Launcher")
}

fn launcher_backup_root(real_workshop_path: &Path) -> PathBuf {
    launcher_root(real_workshop_path)
        .join("backup")
        .join("ProjectZomboid")
}

fn optimization_manifest_path(real_workshop_path: &Path) -> PathBuf {
    launcher_root(real_workshop_path).join("optimizations.json")
}

fn launcher_log_path(real_workshop_path: &Path) -> PathBuf {
    launcher_root(real_workshop_path).join("debug.txt")
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

fn file_sha256(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{:02x}", byte));
    }
    Ok(hex)
}

fn build_manifest(root: &Path) -> io::Result<Vec<ManifestEntry>> {
    let mut files = list_files_recursive(root)?;
    files.sort();
    let mut entries = Vec::with_capacity(files.len());
    for path in files {
        let rel = path
            .strip_prefix(root)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Invalid manifest path"))?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let size = fs::metadata(&path)?.len();
        let hash = file_sha256(&path)?;
        entries.push(ManifestEntry {
            path: rel_str,
            size,
            hash,
        });
    }
    Ok(entries)
}

fn write_manifest(path: &Path, entries: &[ManifestEntry]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest = OptimizationManifest {
        entries: entries.to_vec(),
    };
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(path, json)?;
    Ok(())
}

fn read_manifest(path: &Path) -> io::Result<OptimizationManifest> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

fn manifest_matches_dest(entries: &[ManifestEntry], dst_root: &Path) -> io::Result<bool> {
    if entries.is_empty() {
        return Ok(false);
    }
    for entry in entries {
        let dest_path = dst_root.join(Path::new(&entry.path));
        let meta = match fs::metadata(&dest_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(err),
        };
        if meta.len() != entry.size {
            return Ok(false);
        }
        let hash = file_sha256(&dest_path)?;
        if hash != entry.hash {
            return Ok(false);
        }
    }
    Ok(true)
}

fn manifest_matches_src(entries: &[ManifestEntry], src_root: &Path) -> io::Result<bool> {
    if entries.is_empty() {
        return Ok(false);
    }
    let src_files = list_files_recursive(src_root)?;
    if src_files.len() != entries.len() {
        return Ok(false);
    }
    for entry in entries {
        let src_path = src_root.join(Path::new(&entry.path));
        let meta = match fs::metadata(&src_path) {
            Ok(m) => m,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(err),
        };
        if meta.len() != entry.size {
            return Ok(false);
        }
    }
    Ok(true)
}

fn optimizations_applied(
    src_root: &Path,
    dst_root: &Path,
    manifest_path: &Path,
) -> io::Result<bool> {
    if !dst_root.exists() {
        return Ok(false);
    }
    if manifest_path.exists() {
        let manifest = read_manifest(manifest_path)?;
        if manifest_matches_src(&manifest.entries, src_root)? {
            return manifest_matches_dest(&manifest.entries, dst_root);
        }
        let entries = build_manifest(src_root)?;
        let matches = manifest_matches_dest(&entries, dst_root)?;
        if matches {
            write_manifest(manifest_path, &entries)?;
        }
        return Ok(matches);
    }
    let entries = build_manifest(src_root)?;
    let matches = manifest_matches_dest(&entries, dst_root)?;
    if matches {
        write_manifest(manifest_path, &entries)?;
    }
    Ok(matches)
}

fn copy_dir_replace(
    src_root: &Path,
    dst_root: &Path,
    backup_root: Option<&Path>,
) -> io::Result<(u64, u64, u64)> {
    let mut copied: u64 = 0;
    let mut replaced: u64 = 0;
    let mut backed_up: u64 = 0;
    for s in list_files_recursive(src_root)? {
        let rel = s.strip_prefix(src_root).unwrap();
        let d = dst_root.join(rel);
        if let Some(parent) = d.parent() {
            fs::create_dir_all(parent)?;
        }
        if d.exists() {
            if let Some(backup_root) = backup_root {
                let backup_path = backup_root.join(rel);
                if !backup_path.exists() {
                    if let Some(parent) = backup_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&d, &backup_path)?;
                    backed_up += 1;
                }
            }
            fs::copy(&s, &d)?;
            replaced += 1;
        } else {
            fs::copy(&s, &d)?;
            copied += 1;
        }
    }
    Ok((copied, replaced, backed_up))
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
    let manifest_path = optimization_manifest_path(Path::new(&workshop_path));

    if optimizations_applied(&src, &dest, &manifest_path).map_err(|e| e.to_string())? {
        return Ok(serde_json::json!({
          "already": true,
          "applied": false,
          "source": src.to_string_lossy().to_string(),
          "dest": dest.to_string_lossy().to_string(),
          "manifest": manifest_path.to_string_lossy().to_string()
        }));
    }

    let backup_root = launcher_backup_root(Path::new(&workshop_path));
    fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
    let (copied, replaced, backed_up) =
        copy_dir_replace(&src, &dest, Some(&backup_root)).map_err(|e| e.to_string())?;
    let entries = build_manifest(&src).map_err(|e| e.to_string())?;
    write_manifest(&manifest_path, &entries).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
      "already": false,
      "applied": true,
      "copied": copied,
      "replaced": replaced,
      "backed_up": backed_up,
      "source": src.to_string_lossy().to_string(),
      "dest": dest.to_string_lossy().to_string(),
      "backup_root": backup_root.to_string_lossy().to_string(),
      "manifest": manifest_path.to_string_lossy().to_string()
    }))
}

#[tauri::command]
fn check_optimizations(workshop_path: String) -> Result<bool, String> {
    if workshop_path.is_empty() {
        return Err("Workshop path is empty".into());
    }
    let steam_root =
        steam_root_from_registry().unwrap_or_else(|| "C:/Program Files (x86)/Steam".to_string());
    let src = Path::new(&workshop_path)
        .join("mods")
        .join("13thPandemic")
        .join("ProjectZomboid");
    if !src.exists() {
        return Err(format!("Optimizations folder not found: {}", src.display()));
    }
    let dest = pz_install_dir(&steam_root)
        .ok_or_else(|| "Could not locate ProjectZomboid install directory".to_string())?;
    let manifest_path = optimization_manifest_path(Path::new(&workshop_path));
    optimizations_applied(&src, &dest, &manifest_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_launcher_log(workshop_path: String) -> Result<String, String> {
    if workshop_path.is_empty() {
        return Err("Workshop path is empty".into());
    }
    let log_path = launcher_log_path(Path::new(&workshop_path));
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if !log_path.exists() {
        fs::write(&log_path, "").map_err(|e| e.to_string())?;
    }
    open::that(&log_path).map_err(|e| e.to_string())?;
    Ok(log_path.to_string_lossy().to_string())
}

#[tauri::command]
fn append_launcher_log(workshop_path: String, entry: String) -> Result<(), String> {
    if workshop_path.is_empty() {
        return Err("Workshop path is empty".into());
    }
    let log_path = launcher_log_path(Path::new(&workshop_path));
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    file.write_all(entry.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn write_launcher_log(workshop_path: String, contents: String) -> Result<(), String> {
    if workshop_path.is_empty() {
        return Err("Workshop path is empty".into());
    }
    let log_path = launcher_log_path(Path::new(&workshop_path));
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&log_path, contents).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn play(
    app_handle: tauri::AppHandle,
    appid: String,
    _workshop_id: String,
    workshop_path: String,
    extra_args: Option<Vec<String>>,
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
    let mut command = Command::new(&steam_exe);
    command
        .arg("-applaunch")
        .arg(appid)
        .arg(&cachedir_arg)
        .arg(format!("-connect={}", SERVER_IP))
        .arg(format!("-port={}", SERVER_PORT));
    if let Some(extra_args) = extra_args {
        for arg in extra_args {
            if !arg.trim().is_empty() {
                command.arg(arg);
            }
        }
    }
    command
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
            get_server_status,
            play,
            open_path,
            apply_optimizations,
            resolve_game_root,
            check_optimizations,
            open_launcher_log,
            append_launcher_log,
            write_launcher_log
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
