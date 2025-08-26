
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./app.css";

type Status = 'not_found' | 'ready' | 'playing';

// User-facing messages for each state
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
  const [showDebug, setShowDebug] = useState(false);
  const [steamRoot, setSteamRoot] = useState('');
  const [workshopPath, setWorkshopPath] = useState('');
  const [modsPath, setModsPath] = useState('');
  const [log, setLog] = useState('');
  const [status, setStatus] = useState<Status>('not_found');
  const [busy, setBusy] = useState(false);

  function addLog(s: string) {
    setLog(l => l + `[${new Date().toLocaleTimeString()}] ${s}\n`);
  }


  // Initial detection and refresh logic
  const runAutoDetect = async () => {
    try {
      const res = await invoke<{ steam_root: string, workshop_path: string, mods_path: string }>(
        'auto_detect',
        { workshopId: WORKSHOP_ID }
      );
      setSteamRoot(res.steam_root);
      setWorkshopPath(res.workshop_path);
      setModsPath(res.mods_path);
      addLog(`Steam: ${res.steam_root}`);
      addLog(`Workshop: ${res.workshop_path}`);
      addLog(`Mods: ${res.mods_path}`);
      setStatus(res.workshop_path ? 'ready' : 'not_found');
      if (res.mods_path) {
        try {
          await invoke('cleanup', { modsPath: res.mods_path });
          addLog('Checked and restored mods if needed.');
        } catch (e: unknown) {
          const err = e instanceof Error ? e.message : String(e);
          addLog('Cleanup on auto-detect error: ' + err);
        }
      }
    } catch (e: unknown) {
      const err = e instanceof Error ? e.message : String(e);
      addLog('Auto-detect error: ' + err);
    }
  };

  useEffect(() => {
    // On launcher start, auto-detect and then cleanup
    runAutoDetect();
  }, []);



  // Main button logic
  const handleMainButton = async () => {
    if (status === 'not_found') {
      setBusy(true);
      try {
        await invoke('open_workshop', { workshopId: WORKSHOP_ID });
        addLog('Opened Workshop page in Steam. Please subscribe to the mod, then click Refresh after download completes.');
      } catch (e: unknown) {
        const err = e instanceof Error ? e.message : String(e);
        addLog('Failed to open Workshop: ' + err);
      } finally {
        setBusy(false);
      }
    } else if (status === 'ready') {
      setBusy(true);
      setStatus('playing');
      try {
        const r = await invoke<{ linked: number, backups: number }>('link_all', { workshopPath, modsPath });
        addLog(`Linked ${r.linked} mods; backed up ${r.backups}`);
        await invoke('play', { appid: APPID });
        addLog('Game session ended');
        await invoke('cleanup', { modsPath });
        addLog('Restored old mods');
      } catch (e: unknown) {
        const err = e instanceof Error ? e.message : String(e);
        addLog('Play error: ' + err);
      }
      setStatus('ready');
      setBusy(false);
    }
  };

  // Manual restore button

  const mainLabel = MAIN_LABELS[status];

  return (
    <div className="app-container">
      {/* Default OS title bar restored, custom window controls removed */}

      {/* Debug toggle button, bottom right and less noticeable */}
      <div className="debug-toggle-wrapper">
        <button
          onClick={() => setShowDebug(d => !d)}
          className={`debug-toggle${showDebug ? ' active' : ''}`}
          title={showDebug ? 'Hide Debug Info' : 'Show Debug Info'}
        >
          <span role="img" aria-label="bug">🐞</span>
        </button>
      </div>

      <div className="main-panel">
        <div className="title">PZ 13th Pandemic</div>
        <div className="message">{USER_MESSAGES[status]}</div>
        <button
          className="main-button"
          disabled={busy || status === 'playing'}
          onClick={handleMainButton}
          style={{
            background: busy || status === 'playing' ? '#2a3142' : 'linear-gradient(90deg,#7faaff 0%,#4e7fff 100%)',
            color: busy || status === 'playing' ? '#888' : '#fff',
            boxShadow: busy || status === 'playing' ? 'none' : '0 2px 12px #7faaff44',
            cursor: busy || status === 'playing' ? 'not-allowed' : 'pointer',
          }}
        >
          {mainLabel}
        </button>
        {status === 'not_found' && (
          <button
            className="refresh-button"
            style={{ cursor: busy ? 'not-allowed' : 'pointer', opacity: busy ? 0.5 : 1 }}
            onClick={runAutoDetect}
            disabled={busy}
          >
            Refresh
          </button>
        )}
      </div>

      {showDebug && (
        <div className="debug-panel">
          <div className="debug-panel-grid">
            <div>Steam root</div>
            <input value={steamRoot} onChange={e => setSteamRoot(e.target.value)} />
            <div>Workshop</div>
            <input value={workshopPath} onChange={e => setWorkshopPath(e.target.value)} />
            <div>Mods dir</div>
            <input value={modsPath} onChange={e => setModsPath(e.target.value)} />
          </div>
          <pre className="debug-log">{log}</pre>
        </div>
      )}
    </div>
  );
}

export default App;
