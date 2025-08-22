import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

const APPID = '108600'
const WORKSHOP_ID = '3487726294'


function App() {
  const [steamRoot, setSteamRoot] = useState('')
  const [workshopPath, setWorkshopPath] = useState('')
  const [modsPath, setModsPath] = useState('')
  const [log, setLog] = useState('')
  const [busy, setBusy] = useState(false)

  function addLog(s:string){ setLog(l=>l + `[${new Date().toLocaleTimeString()}] ${s}\n`) }

  useEffect(()=>{
    (async()=>{
      const res = await invoke<{steam_root:string,workshop_path:string,mods_path:string}>('auto_detect', { appid: APPID, workshopId: WORKSHOP_ID })
      setSteamRoot(res.steam_root); setWorkshopPath(res.workshop_path); setModsPath(res.mods_path)
      addLog(`Steam: ${res.steam_root}`)
      addLog(`Workshop: ${res.workshop_path}`)
      addLog(`Mods: ${res.mods_path}`)
    })()
  },[])

  const openWorkshop = async()=>{
    await invoke('open_workshop', { workshopId: WORKSHOP_ID })
    addLog('Opened Workshop page in Steam')
  }

  const waitDownload = async()=>{
    setBusy(true)
    try{ await invoke('wait_for_download', { workshopPath }) ; addLog('Download complete') }
    catch(e:any){ addLog('Wait error: ' + e) }
    setBusy(false)
  }

  const linkMods = async()=>{
    setBusy(true)
    try{ const r = await invoke<{linked:number,backups:number}>('link_all', { workshopPath, modsPath }) ; addLog(`Linked ${r.linked} mods; backed up ${r.backups}`)}
    catch(e:any){ addLog('Link error: ' + e) }
    setBusy(false)
  }

  const play = async()=>{
    setBusy(true)
    try{ await invoke('play', { appid: APPID }); addLog('Game session ended') }
    catch(e:any){ addLog('Play error: ' + e) }
    setBusy(false)
  }

  const cleanup = async()=>{
    setBusy(true)
    try{ const r = await invoke<{removed:number,restored:number}>('cleanup', { modsPath }); addLog(`Cleanup: removed ${r.removed}, restored ${r.restored}`) }
    catch(e:any){ addLog('Cleanup error: ' + e) }
    setBusy(false)
  }

  return (
    <div style={{fontFamily:'system-ui', padding:16, maxWidth:900}}>
      <h2>PZ 13th Pandemic Launcher (Tauri)</h2>
      <div style={{display:'grid', gridTemplateColumns:'140px 1fr', gap:8, alignItems:'center'}}>
        <div>Steam root</div><input value={steamRoot} onChange={e=>setSteamRoot(e.target.value)} />
        <div>Workshop</div><input value={workshopPath} onChange={e=>setWorkshopPath(e.target.value)} />
        <div>Mods dir</div><input value={modsPath} onChange={e=>setModsPath(e.target.value)} />
      </div>
      <div style={{marginTop:12, display:'flex', gap:8, flexWrap:'wrap'}}>
        <button disabled={busy} onClick={openWorkshop}>Open Workshop</button>
        <button disabled={busy} onClick={waitDownload}>Wait for Download</button>
        <button disabled={busy} onClick={linkMods}>Create Links</button>
        <button disabled={busy} onClick={play}>Play</button>
        <button disabled={busy} onClick={cleanup}>Clean Links</button>
      </div>
      <pre style={{marginTop:16, background:'#111', color:'#ddd', padding:12, height:320, overflow:'auto'}}>{log}</pre>
    </div>
  )
}

export default App;
