import { type SVGProps, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import "./app.css";

type Status = "not_found" | "ready" | "playing";

const USER_MESSAGES: Record<Status, string> = {
  not_found:
    "Mod not found. Click Download to open the Steam Workshop page and subscribe. Once the mod finishes downloading in Steam, click Refresh.",
  ready:
    "Ready to play! Click Play to start Project Zomboid with the modpack and join the server.",
  playing: "Game is running. Please wait until the session ends.",
};

const APPID = "108600";
const WORKSHOP_ID = "3487726294";

const MAIN_LABELS: Record<Status, string> = {
  not_found: "Download",
  ready: "Play",
  playing: "Playing...",
};

function SettingsIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      {...props}
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 0 0 2.564 1.066c1.543-.89 3.31.877 2.42 2.42a1.724 1.724 0 0 0 1.066 2.564c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 0 0-1.066 2.564c.89 1.543-.877 3.31-2.42 2.42a1.724 1.724 0 0 0-2.564 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 0 0-2.564-1.066c-1.543.89-3.31-.877-2.42-2.42a1.724 1.724 0 0 0-1.066-2.564c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 0 0 1.066-2.564c-.89-1.543.877-3.31 2.42-2.42.996.575 2.273.12 2.564-1.066Z" />
      <path d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
    </svg>
  );
}

function App() {
  const [showSettings, setShowSettings] = useState(false);
  const [steamRoot, setSteamRoot] = useState("");
  const [workshopPath, setWorkshopPath] = useState("");
  const [cachedirPath, setCachedirPath] = useState("");
  const [gameRootPath, setGameRootPath] = useState("");
  const [log, setLog] = useState("");
  const [status, setStatus] = useState<Status>("not_found");
  const [busy, setBusy] = useState(false);

  const addLog = useCallback((entry: string) => {
    setLog(prev => `${prev}[${new Date().toLocaleTimeString()}] ${entry}\n`);
  }, []);

  const openFolder = async (path?: string) => {
    if (!path) return;
    try {
      await invoke("open_path", { path });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addLog(`Open folder failed: ${msg}`);
    }
  };

  const runAutoDetect = async () => {
    try {
      const res = await invoke<{ steam_root: string; workshop_path: string }>("auto_detect", {
        workshopId: WORKSHOP_ID,
      });
      setSteamRoot(res.steam_root);
      setWorkshopPath(res.workshop_path);
      addLog(`Steam: ${res.steam_root}`);
      addLog(`Workshop: ${res.workshop_path || "(not found)"}`);
      const resolvedStatus = res.workshop_path ? "ready" : "not_found";
      setStatus(resolvedStatus);
      const cz = res.workshop_path
        ? `${res.workshop_path}\\mods\\13thPandemic\\Zomboid`
        : "";
      setCachedirPath(cz);
      if (cz) {
        addLog(`Cachedir: ${cz}`);
      }
      try {
        const gameRoot = await invoke<string>("resolve_game_root");
        setGameRootPath(gameRoot);
      } catch {
        setGameRootPath("");
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addLog(`Auto-detect error: ${msg}`);
    }
  };

  useEffect(() => {
    runAutoDetect();
  }, []);

  useEffect(() => {
    let disposed = false;
    const unlistenFns: UnlistenFn[] = [];

    const attach = async () => {
      try {
        const unlistenLaunch = await listen<{ cachedir?: string }>(
          "pz-session-launched",
          event => {
            const info = event.payload?.cachedir ?? "(unknown)";
            addLog(`Game launch initiated (cachedir: ${info})`);
          }
        );
        if (disposed) {
          unlistenLaunch();
        } else {
          unlistenFns.push(unlistenLaunch);
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        addLog(`Failed to bind launch listener: ${msg}`);
      }

      try {
        const unlistenEnded = await listen<{ cachedir?: string; found?: boolean }>(
          "pz-session-ended",
          event => {
            const info = event.payload?.cachedir ?? "(unknown)";
            if (event.payload?.found === false) {
              addLog(`Game session ended (process not detected). Cachedir: ${info}`);
            } else {
              addLog(`Game session ended. Cachedir: ${info}`);
            }
            setStatus(prev => (prev === "playing" ? "ready" : prev));
            setBusy(false);
          }
        );
        if (disposed) {
          unlistenEnded();
        } else {
          unlistenFns.push(unlistenEnded);
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        addLog(`Failed to bind session listener: ${msg}`);
        setStatus(prev => (prev === "playing" ? "ready" : prev));
        setBusy(false);
      }
    };

    attach();

    return () => {
      disposed = true;
      for (const unlisten of unlistenFns) {
        try {
          unlisten();
        } catch {
          // ignore
        }
      }
    };
  }, [addLog]);

  const handleMainButton = async () => {
    if (status === "not_found") {
      setBusy(true);
      try {
        await invoke("open_workshop", { workshopId: WORKSHOP_ID });
        addLog(
          "Opened Workshop page in Steam. Subscribe to the mod, then click Refresh after download completes."
        );
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        addLog(`Failed to open Workshop: ${msg}`);
      } finally {
        setBusy(false);
      }
      return;
    }

    if (status === "ready") {
      setBusy(true);
      setStatus("playing");
      try {
        addLog(`Launching with cachedir: ${cachedirPath || "(auto)"}`);
        await invoke("play", {
          appid: APPID,
          workshopId: WORKSHOP_ID,
          workshopPath,
        });
        addLog("Launch command sent. Waiting for Project Zomboid to exit...");
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        addLog(`Play error: ${msg}`);
        setStatus("ready");
        setBusy(false);
      }
    }
  };

  const handleApplyOptimizations = async () => {
    if (!workshopPath) {
      await message("Workshop path not detected yet. Click Refresh.");
      return;
    }
    try {
      setBusy(true);
      const res = await invoke<{
        already?: boolean;
        applied?: boolean;
        copied?: number;
        replaced?: number;
        source: string;
        dest: string;
      }>("apply_optimizations", { workshopPath });
      if (res.already) {
        await message("Optimizations already applied.");
      } else {
        const copied = res.copied ?? 0;
        const replaced = res.replaced ?? 0;
        await message(
          `Optimizations applied. Copied: ${copied}, Replaced: ${replaced}.`
        );
        addLog(
          `Optimizations applied to ${res.dest} from ${res.source}. Copied ${copied}, replaced ${replaced}.`
        );
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      await message(`Optimization apply failed: ${msg}`);
    } finally {
      setBusy(false);
    }
  };

  const mainLabel = MAIN_LABELS[status];

  return (
    <div className="app-container">
      <div className="main-panel">
        <div className="title">PZ 13th Pandemic</div>
        <div className="message">{USER_MESSAGES[status]}</div>
        <div style={{ display: "flex", gap: 10 }}>
          <button
            className="main-button"
            disabled={busy || status === "playing"}
            onClick={handleMainButton}
            style={{
              background:
                busy || status === "playing"
                  ? "#2a3142"
                  : "linear-gradient(90deg,#7faaff 0%,#4e7fff 100%)",
              color: busy || status === "playing" ? "#888" : "#fff",
              boxShadow:
                busy || status === "playing" ? "none" : "0 2px 12px #7faaff44",
              cursor: busy || status === "playing" ? "not-allowed" : "pointer",
            }}
          >
            {mainLabel}
          </button>
        </div>
        {status === "not_found" && (
          <button
            className="refresh-button"
            style={{ cursor: busy ? "not-allowed" : "pointer", opacity: busy ? 0.5 : 1 }}
            onClick={runAutoDetect}
            disabled={busy}
          >
            Refresh
          </button>
        )}
        <button
          className="refresh-button"
          onClick={handleApplyOptimizations}
          disabled={busy || status !== "ready"}
        >
          Apply Optimizations
        </button>
      </div>

      {showSettings && (
        <div className="debug-panel">
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: 8,
            }}
          >
            <div style={{ fontWeight: 600, color: "#7faaff" }}>Settings & Console</div>
            <button
              className="icon-button"
              onClick={() => setShowSettings(false)}
              title="Close"
            >
              X
            </button>
          </div>
          <div className="debug-panel-grid">
            <div>Steam root</div>
            <div style={{ display: "flex", gap: 6 }}>
              <input value={steamRoot} onChange={event => setSteamRoot(event.target.value)} />
              <button
                className="icon-button"
                onClick={() => openFolder(steamRoot)}
                title="Open folder"
              >
                Open
              </button>
            </div>

            <div>Workshop parent</div>
            <div style={{ display: "flex", gap: 6 }}>
              <input
                value={
                  workshopPath
                    ? workshopPath.split(/[\\/]/).slice(0, -1).join("\\")
                    : ""
                }
                onChange={() => {}}
                readOnly
              />
              <button
                className="icon-button"
                onClick={() =>
                  openFolder(
                    workshopPath
                      ? workshopPath.split(/[\\/]/).slice(0, -1).join("\\")
                      : ""
                  )
                }
                title="Open folder"
              >
                Open
              </button>
            </div>

            <div>Cachedir location</div>
            <div style={{ display: "flex", gap: 6 }}>
              <input value={cachedirPath} onChange={event => setCachedirPath(event.target.value)} />
              <button
                className="icon-button"
                onClick={() => openFolder(cachedirPath)}
                title="Open folder"
              >
                Open
              </button>
            </div>

            <div>Game root</div>
            <div style={{ display: "flex", gap: 6 }}>
              <input value={gameRootPath} onChange={() => {}} readOnly />
              <button
                className="icon-button"
                onClick={() => openFolder(gameRootPath)}
                title="Open folder"
              >
                Open
              </button>
            </div>
          </div>
          <pre className="debug-log">{log}</pre>
        </div>
      )}

      <div className="debug-toggle-wrapper">
        <button
          onClick={() => setShowSettings(value => !value)}
          className={`debug-toggle${showSettings ? " active" : ""}`}
          title={showSettings ? "Hide Settings & Console" : "Show Settings & Console"}
          aria-label={showSettings ? "Hide Settings & Console" : "Show Settings & Console"}
          aria-pressed={showSettings}
        >
          <SettingsIcon aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}

export default App;