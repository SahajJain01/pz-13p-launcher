import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import "./app.css";

type Status = 'not_found' | 'ready' | 'playing';

const USER_MESSAGES: Record<Status, string> = {
  not_found:
    'Mod not found. Click Download to open the Steam Workshop page and subscribe. Once the mod finishes downloading in Steam, click Refresh.',
  ready:
    'Ready to play! Click Play to start Project Zomboid with the modpack and join the server.',
  playing: 'Game is running. Please wait until the session ends.',
};

const APPID = '108600';
const WORKSHOP_ID = '3487726294';

const MAIN_LABELS: Record<Status, string> = {
  not_found: 'Download',
  ready: 'Play',
  playing: 'Playing...'
};

function App() {
  const [showSettings, setShowSettings] = useState(false);
  const [showConsole, setShowConsole] = useState(false);
  const [steamRoot, setSteamRoot] = useState('');
  const [workshopPath, setWorkshopPath] = useState('');
  const [modsPath, setModsPath] = useState('');
  const [modsRealPath, setModsRealPath] = useState('');
  const [cachedirPath, setCachedirPath] = useState('');
  const [log, setLog] = useState('');
  const [status, setStatus] = useState<Status>('not_found');
  const [busy, setBusy] = useState(false);

  function addLog(s: string) {
    setLog(l => l + `[${new Date().toLocaleTimeString()}] ${s}\n`);
  }

  const openFolder = async (path?: string) => {
    if (!path) return;
    try { await invoke('open_path', { path }); } catch (e) {
      const err = e instanceof Error ? e.message : String(e);
      addLog('Open folder failed: ' + err);
    }
  };

  const runAutoDetect = async () => {
    try {
      const res = await invoke<{ steam_root: string, workshop_path: string, mods_path: string }>('auto_detect', { workshopId: WORKSHOP_ID });
      setSteamRoot(res.steam_root);
      setWorkshopPath(res.workshop_path);
      setModsPath(res.mods_path);
      addLog(`Steam: ${res.steam_root}`);
      addLog(`Workshop: ${res.workshop_path}`);
      addLog(`Mods (user): ${res.mods_path}`);
      setStatus(res.workshop_path ? 'ready' : 'not_found');
      const cz = res.workshop_path ? `${res.workshop_path}\\mods\\13thPandemic\\Zomboid` : '';
      setCachedirPath(cz);
      try {
        const realMods = await invoke<string>('resolve_workshop_mods', { workshopPath: res.workshop_path });
        setModsRealPath(realMods);
      } catch {
        setModsRealPath(res.workshop_path ? `${res.workshop_path}\\mods\\13thPandemic\\Zomboid\\Mods` : '');
      }
    } catch (e: unknown) {
      addLog('Auto-detect error: ' + (e instanceof Error ? e.message : String(e)));
    }
  };

  useEffect(() => { runAutoDetect(); }, []);

  const handleMainButton = async () => {
    if (status === 'not_found') {
      setBusy(true);
      try {
        await invoke('open_workshop', { workshopId: WORKSHOP_ID });
        addLog('Opened Workshop page in Steam. Subscribe to the mod, then click Refresh after download completes.');
      } catch (e: unknown) {
        addLog('Failed to open Workshop: ' + (e instanceof Error ? e.message : String(e)));
      } finally { setBusy(false); }
    } else if (status === 'ready') {
      setBusy(true);
      setStatus('playing');
      try {
        addLog(`Launching with cachedir: ${cachedirPath || '(auto)'}`);
        await invoke('play', { appid: APPID, workshopId: WORKSHOP_ID, workshopPath });
        addLog('Game session ended');
      } catch (e: unknown) {
        addLog('Play error: ' + (e instanceof Error ? e.message : String(e)));
      }
      setStatus('ready');
      setBusy(false);
    }
  };

  const handleMoveWorkshop = async () => {
    if (!workshopPath) { addLog('Workshop path not detected yet. Click Refresh.'); return; }
    try {
      setBusy(true);
      const picked = await openDialog({ directory: true, multiple: false, title: 'Pick destination for the mods folder' });
      const destDir = Array.isArray(picked) ? picked[0] : picked;
      if (!destDir) { addLog('Move canceled'); return; }
      const res = await invoke<{ new_path: string }>('move_workshop', { workshopPath, workshopId: WORKSHOP_ID, destDir: destDir as string });
      addLog(`Moved mods folder to: ${(res as any).new_path}`);
      addLog('Updated junction at workshop/mods/13thPandemic/Zomboid/Mods');
      setModsRealPath((res as any).new_path);
      setCachedirPath(`${workshopPath}\\mods\\13thPandemic\\Zomboid`);
      await runAutoDetect();
    } catch (e: unknown) {
      addLog('Move failed: ' + (e instanceof Error ? e.message : String(e)));
    } finally { setBusy(false); }
  };

  const handleRestoreWorkshop = async () => {
    if (!workshopPath) { addLog('Workshop path not detected yet. Click Refresh.'); return; }
    try {
      setBusy(true);
      const res = await invoke<{ restored: boolean, path: string }>('restore_workshop', { workshopPath, workshopId: WORKSHOP_ID });
      if ((res as any).restored) {
        addLog('Restored mods to workshop and removed junction');
        setModsRealPath(`${workshopPath}\\mods\\13thPandemic\\Zomboid\\Mods`);
        setCachedirPath(`${workshopPath}\\mods\\13thPandemic\\Zomboid`);
        await runAutoDetect();
      } else { addLog('Nothing to restore'); }
    } catch (e: unknown) {
      addLog('Restore failed: ' + (e instanceof Error ? e.message : String(e)));
    } finally { setBusy(false); }
  };

  const mainLabel = MAIN_LABELS[status];

  return (
    <div className="app-container">
      <div className="main-panel">
        <div className="title">PZ 13th Pandemic</div>
        <div className="message">{USER_MESSAGES[status]}</div>
        <div style={{ display: 'flex', gap: 10 }}>
          <button className="main-button" disabled={busy || status === 'playing'} onClick={handleMainButton}
            style={{
              background: busy || status === 'playing' ? '#2a3142' : 'linear-gradient(90deg,#7faaff 0%,#4e7fff 100%)',
              color: busy || status === 'playing' ? '#888' : '#fff',
              boxShadow: busy || status === 'playing' ? 'none' : '0 2px 12px #7faaff44',
              cursor: busy || status === 'playing' ? 'not-allowed' : 'pointer',
            }}>
            {mainLabel}
          </button>
          <button className="main-button secondary" disabled={busy || status === 'playing'} onClick={() => setShowSettings(s => !s)} title={showSettings ? 'Hide Settings' : 'Show Settings'}>
            Settings
          </button>
        </div>
        {status === 'not_found' && (
          <button className="refresh-button" style={{ cursor: busy ? 'not-allowed' : 'pointer', opacity: busy ? 0.5 : 1 }} onClick={runAutoDetect} disabled={busy}>Refresh</button>
        )}
      </div>

      {showSettings && (
        <div className="debug-panel">
          <div className="debug-panel-grid">
            <div>Steam root</div>
            <div style={{display:'flex', gap:6}}>
              <input value={steamRoot} onChange={e => setSteamRoot(e.target.value)} />
              <button className="icon-button" onClick={() => openFolder(steamRoot)} title="Open folder">📂</button>
            </div>
            <div>Steam workshop folder for PZ</div>
            <div style={{display:'flex', gap:6}}>
              <input value={workshopPath ? workshopPath.split(/[/\\]/).slice(0,-1).join('\\') : ''} onChange={() => {}} readOnly />
              <button className="icon-button" onClick={() => openFolder(workshopPath ? workshopPath.split(/[/\\]/).slice(0,-1).join('\\') : '')} title="Open folder">📂</button>
            </div>
            <div>Cachedir location</div>
            <div style={{display:'flex', gap:6}}>
              <input value={cachedirPath} onChange={e => setCachedirPath(e.target.value)} />
              <button className="icon-button" onClick={() => openFolder(cachedirPath)} title="Open folder">📂</button>
            </div>
            <div>Current mod location</div>
            <div style={{display:'flex', gap:6}}>
              <input value={modsRealPath || (workshopPath ? `${workshopPath}\\mods\\13thPandemic\\Zomboid\\Mods` : '')} onChange={e => setModsRealPath(e.target.value)} />
              <button className="icon-button" onClick={() => openFolder(modsRealPath || (workshopPath ? `${workshopPath}\\mods\\13thPandemic\\Zomboid\\Mods` : ''))} title="Open folder">📂</button>
            </div>
          </div>
          <div style={{ display:'flex', gap: 8, marginTop: 8 }}>
            <button className="refresh-button" onClick={handleMoveWorkshop} disabled={busy || status === 'playing'}>Move Mod Folder</button>
            <button className="refresh-button" onClick={handleRestoreWorkshop} disabled={busy || status === 'playing'}>Restore to Steam</button>
          </div>
        </div>
      )}

      <div className="debug-panel" style={{ display: showConsole ? 'block' : 'none' }}>
        <pre className="debug-log">{log}</pre>
      </div>
    </div>
  );
}

export default App;

