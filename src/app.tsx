import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
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
  const [steamRoot, setSteamRoot] = useState('');
  const [workshopPath, setWorkshopPath] = useState('');
  const [cachedirPath, setCachedirPath] = useState('');
  const [gameRootPath, setGameRootPath] = useState('');
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
      addLog(`Steam: ${res.steam_root}`);
      addLog(`Workshop: ${res.workshop_path}`);
      addLog(`Mods (user): ${res.mods_path}`);
      setStatus(res.workshop_path ? 'ready' : 'not_found');
      const cz = res.workshop_path ? `${res.workshop_path}\\mods\\13thPandemic\\Zomboid` : '';
      setCachedirPath(cz);
      try {
        const gameRoot = await invoke<string>('resolve_game_root');
        setGameRootPath(gameRoot);
      } catch {
        setGameRootPath('');
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

  const handleApplyOptimizations = async () => {
    if (!workshopPath) {
      await message('Workshop path not detected yet. Click Refresh.');
      return;
    }
    try {
      setBusy(true);
      const res = await invoke<{ already?: boolean; applied?: boolean; copied?: number; replaced?: number; source: string; dest: string }>(
        'apply_optimizations',
        { workshopPath }
      );
      if ((res as any).already) {
        await message('Optimizations already applied.');
      } else {
        const copied = (res as any).copied ?? 0;
        const replaced = (res as any).replaced ?? 0;
        await message(`Optimizations applied. Copied: ${copied}, Replaced: ${replaced}.`);
        addLog(`Optimizations applied to ${(res as any).dest} from ${(res as any).source}. Copied ${copied}, replaced ${replaced}.`);
      }
    } catch (e: unknown) {
      await message('Optimization apply failed: ' + (e instanceof Error ? e.message : String(e)));
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
        </div>
        {status === 'not_found' && (
          <button className="refresh-button" style={{ cursor: busy ? 'not-allowed' : 'pointer', opacity: busy ? 0.5 : 1 }} onClick={runAutoDetect} disabled={busy}>Refresh</button>
        )}
        <button className="refresh-button" onClick={handleApplyOptimizations} disabled={busy || status !== 'ready'}>Apply Optimizations</button>
      </div>

      {showSettings && (
        <div className="debug-panel">
          <div style={{ display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom: 8 }}>
            <div style={{ fontWeight: 600, color:'#7faaff' }}>Settings & Console</div>
            <button className="icon-button" onClick={() => setShowSettings(false)} title="Close">✖</button>
          </div>
          <div className="debug-panel-grid">
            <div>Steam root</div>
            <div style={{display:'flex', gap:6}}>
              <input value={steamRoot} onChange={e => setSteamRoot(e.target.value)} />
              <button className="icon-button" onClick={() => openFolder(steamRoot)} title="Open folder">Open</button>
            </div>

            <div>Workshop parent</div>
            <div style={{display:'flex', gap:6}}>
              <input value={workshopPath ? workshopPath.split(/[/\\]/).slice(0,-1).join('\\') : ''} onChange={() => {}} readOnly />
              <button className="icon-button" onClick={() => openFolder(workshopPath ? workshopPath.split(/[/\\]/).slice(0,-1).join('\\') : '')} title="Open folder">Open</button>
            </div>

            <div>Cachedir location</div>
            <div style={{display:'flex', gap:6}}>
              <input value={cachedirPath} onChange={e => setCachedirPath(e.target.value)} />
              <button className="icon-button" onClick={() => openFolder(cachedirPath)} title="Open folder">Open</button>
            </div>

            <div>Game root</div>
            <div style={{display:'flex', gap:6}}>
              <input value={gameRootPath} onChange={() => {}} readOnly />
              <button className="icon-button" onClick={() => openFolder(gameRootPath)} title="Open folder">Open</button>
            </div>
          </div>
          <pre className="debug-log">{log}</pre>
        </div>
      )}

      <div className="debug-toggle-wrapper">
        <button
          onClick={() => setShowSettings(s => !s)}
          className={`debug-toggle${showSettings ? ' active' : ''}`}
          title={showSettings ? 'Hide Settings & Console' : 'Show Settings & Console'}>
          ⚙️
        </button>
      </div>
    </div>
  );
}

export default App;
