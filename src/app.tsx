
import { useEffect, useState, useRef } from "react";
// User-facing messages for each state
const USER_MESSAGES: Record<string, string> = {
  not_found: 'Mod not found. Click Download to open the Steam Workshop page and subscribe. Once the mod finishes downloading in Steam, click Refresh.',
  ready: 'Ready to play! Click Play to start Project Zomboid with the modpack.',
  playing: 'Game is running. Please wait until the session ends.',
};
import { invoke } from "@tauri-apps/api/core";
import "./app.css";


const APPID = '108600';
const WORKSHOP_ID = '3487726294';

type Status = 'not_found' | 'downloading' | 'ready' | 'playing';



function App() {
  const [showDebug, setShowDebug] = useState(false);
  const [steamRoot, setSteamRoot] = useState('');
  const [workshopPath, setWorkshopPath] = useState('');
  const [modsPath, setModsPath] = useState('');
  const [log, setLog] = useState('');
  const [status, setStatus] = useState<Status>('not_found');
  const [busy, setBusy] = useState(false);
  // const [progress, setProgress] = useState<number|null>(null); // Not used
  const downloadInterval = useRef<ReturnType<typeof setInterval> | null>(null);

  function addLog(s: string) {
    setLog(l => l + `[${new Date().toLocaleTimeString()}] ${s}\n`);
  }


  // Initial detection and refresh logic
  const runAutoDetect = async () => {
    const res = await invoke<{ steam_root: string, workshop_path: string, mods_path: string }>('auto_detect', { workshopId: WORKSHOP_ID });
    setSteamRoot(res.steam_root);
    setWorkshopPath(res.workshop_path);
    setModsPath(res.mods_path);
    addLog(`Steam: ${res.steam_root}`);
    addLog(`Workshop: ${res.workshop_path}`);
    addLog(`Mods: ${res.mods_path}`);
    if (!res.workshop_path) {
      setStatus('not_found');
    } else {
      setStatus('ready');
    }
    // Run cleanup after modsPath is set
    if (res.mods_path) {
      try {
        await invoke('cleanup', { modsPath: res.mods_path });
        addLog('Checked and restored mods if needed.');
      } catch (e: any) {
        addLog('Cleanup on auto-detect error: ' + e);
      }
    }
  };

  useEffect(() => {
    // On launcher start, auto-detect and then cleanup
    runAutoDetect();
    return () => {
      if (downloadInterval.current) clearInterval(downloadInterval.current);
    };
  }, []);



  // Main button logic
  const handleMainButton = async () => {
    if (status === 'not_found') {
      setBusy(true);
      await invoke('open_workshop', { workshopId: WORKSHOP_ID });
      addLog('Opened Workshop page in Steam. Please subscribe to the mod, then click Refresh after download completes.');
      setBusy(false);
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
      } catch (e: any) {
        addLog('Play error: ' + e);
      }
      setStatus('ready');
      setBusy(false);
    }
  };

  // Manual restore button

  let mainLabel = 'Play';
  if (status === 'not_found') mainLabel = 'Download';
  else if (status === 'playing') mainLabel = 'Playing...';

  return (
    <div style={{
      fontFamily: 'Segoe UI, Arial, sans-serif',
      height: '100vh',
      width: '100vw',
      background: 'radial-gradient(ellipse at 60% 0%, #23293a 60%, #181c24 100%)',
      color: '#e3e6ed',
      padding: 0,
      margin: 0,
      maxWidth: '100vw',
      maxHeight: '100vh',
      position: 'relative',
      overflow: 'hidden',
      boxSizing: 'border-box',
      border: 'none',
      boxShadow: 'none',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
    }}>
      {/* Default OS title bar restored, custom window controls removed */}

      {/* Debug toggle button, bottom right and less noticeable */}
      <div style={{
        position: 'fixed',
        bottom: 24,
        right: 24,
        zIndex: 19,
      }}>
        <button
          onClick={() => setShowDebug(d => !d)}
          style={{
            width: 28,
            height: 28,
            borderRadius: 7,
            background: showDebug ? '#23293a' : 'transparent',
            color: '#7faaff',
            border: '1.5px dashed #2a3142',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 16,
            cursor: 'pointer',
            boxShadow: 'none',
            opacity: 0.5,
            transition: 'background 0.2s, color 0.2s, opacity 0.2s',
            marginRight: 0,
            padding: 0,
          }}
          title={showDebug ? 'Hide Debug Info' : 'Show Debug Info'}
        >
          <span role="img" aria-label="bug">🐞</span>
        </button>
      </div>

      <div style={{
        maxWidth: 420,
        margin: 0,
        background: 'none',
        borderRadius: 0,
        boxShadow: 'none',
        border: 'none',
        padding: '36px 32px 32px 32px',
        position: 'relative',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        outline: 'none',
      }}>
        <div style={{ fontSize: 28, fontWeight: 700, letterSpacing: 1, marginBottom: 8, color: '#7faaff', textShadow: '0 2px 8px #0008' }}>
          PZ 13th Pandemic
        </div>
        <div style={{ fontSize: 15, color: '#b6c2e0', marginBottom: 28, textAlign: 'center', lineHeight: 1.5, minHeight: 48, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          {USER_MESSAGES[status]}
        </div>
        <button
          disabled={busy || status === 'playing'}
          onClick={handleMainButton}
          style={{
            minWidth: 180,
            height: 48,
            fontSize: 20,
            fontWeight: 600,
            padding: 0,
            borderRadius: 10,
            background: busy || status === 'playing' ? '#2a3142' : 'linear-gradient(90deg,#7faaff 0%,#4e7fff 100%)',
            color: busy || status === 'playing' ? '#888' : '#fff',
            border: 'none',
            boxShadow: busy || status === 'playing' ? 'none' : '0 2px 12px #7faaff44',
            marginBottom: 10,
            marginTop: 8,
            cursor: busy || status === 'playing' ? 'not-allowed' : 'pointer',
            transition: 'background 0.2s, color 0.2s',
          }}
        >
          {mainLabel}
        </button>
        {status === 'not_found' && (
          <button
            style={{
              fontSize: 14,
              padding: '6px 18px',
              borderRadius: 7,
              background: '#23293a',
              color: '#7faaff',
              border: '1.5px solid #2a3142',
              marginLeft: 0,
              marginTop: 6,
              marginBottom: 2,
              cursor: busy ? 'not-allowed' : 'pointer',
              opacity: busy ? 0.5 : 1,
              fontWeight: 500,
              letterSpacing: 0.5,
              boxShadow: '0 1px 4px #0004',
              transition: 'background 0.2s, color 0.2s',
            }}
            onClick={runAutoDetect}
            disabled={busy}
          >
            Refresh
          </button>
        )}
      </div>

      {showDebug && (
        <div style={{
          position: 'fixed',
          bottom: 64,
          right: 32,
          width: 420,
          maxWidth: '90vw',
          border: '2px solid #3a4660',
          borderRadius: 16,
          background: 'linear-gradient(135deg, #23293a 80%, #2a3142 100%)',
          color: '#eee',
          padding: 20,
          zIndex: 100,
          boxShadow: '0 8px 32px #000b, 0 1.5px 0 #7faaff44',
          filter: 'drop-shadow(0 2px 12px #0008)',
        }}>
          <div style={{ display: 'grid', gridTemplateColumns: '140px 1fr', gap: 8, alignItems: 'center', marginBottom: 8 }}>
            <div>Steam root</div><input value={steamRoot} onChange={e => setSteamRoot(e.target.value)} style={{ background: '#181c24', color: '#fff', border: '1px solid #2a3142', borderRadius: 5, padding: 4 }} />
            <div>Workshop</div><input value={workshopPath} onChange={e => setWorkshopPath(e.target.value)} style={{ background: '#181c24', color: '#fff', border: '1px solid #2a3142', borderRadius: 5, padding: 4 }} />
            <div>Mods dir</div><input value={modsPath} onChange={e => setModsPath(e.target.value)} style={{ background: '#181c24', color: '#fff', border: '1px solid #2a3142', borderRadius: 5, padding: 4 }} />
          </div>
          <pre style={{ background: '#181c24', color: '#7faaff', padding: 12, height: 200, overflow: 'auto', borderRadius: 6 }}>{log}</pre>
        </div>
      )}
    </div>
  );
}

export default App;
