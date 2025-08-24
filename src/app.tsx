
import { useEffect, useState } from "react";
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
  const [workshopPath, setWorkshopPath] = useState('');
  const [modsPath, setModsPath] = useState('');
  const [status, setStatus] = useState<Status>('not_found');
  const [busy, setBusy] = useState(false);



  // Initial detection and refresh logic
  const runAutoDetect = async () => {
    const res = await invoke<{ steam_root: string, workshop_path: string, mods_path: string }>('auto_detect', { appid: APPID, workshopId: WORKSHOP_ID });
  // removed setSteamRoot
    setWorkshopPath(res.workshop_path);
    setModsPath(res.mods_path);
    if (!res.workshop_path) {
      setStatus('not_found');
    } else {
      setStatus('ready');
    }
  };

  useEffect(() => {
  // On launcher start, always attempt to auto-detect
  runAutoDetect();
  return () => {};
  }, []);



  // Main button logic
  const handleMainButton = async () => {
    if (status === 'not_found') {
      setBusy(true);
      await invoke('open_workshop', { workshopId: WORKSHOP_ID });
      setBusy(false);
    } else if (status === 'ready') {
      setBusy(true);
      setStatus('playing');
      try {
  await invoke<{ linked: number, backups: number }>('link_all', { workshopPath, modsPath });
        await invoke('play', { appid: APPID });
      } catch (e: any) {
      }
      setStatus('ready');
      setBusy(false);
    }
  };


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


    </div>
  );
}

export default App;
