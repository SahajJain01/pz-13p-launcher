import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import App from "./app";
import { invoke } from "@tauri-apps/api/core";
import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";

const listeners: Record<string, (event: { payload?: Record<string, unknown> }) => void> = {};
const listenMock = vi.fn(
  (
    event: string,
    handler: (event: { payload?: Record<string, unknown> }) => void
  ): Promise<() => void> => {
    listeners[event] = handler;
    return Promise.resolve(() => {
      delete listeners[event];
    });
  }
);

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ message: vi.fn(() => Promise.resolve()) }));

describe("App", () => {
  let invokeMock: Mock;

  const setupInvokeMock = ({
    autoDetectQueue = [{ steam_root: "", workshop_path: "" }],
    gameRoot = "",
    optimizationsApplied = false,
    serverStatus = { ip: "16.102.174.79", ping_ms: 50 },
  }: {
    autoDetectQueue?: Array<{ steam_root: string; workshop_path: string }>;
    gameRoot?: string;
    optimizationsApplied?: boolean;
    serverStatus?: { ip: string; ping_ms: number | null };
  } = {}) => {
    const queue = [...autoDetectQueue];
    let lastAutoDetect = queue[0] ?? { steam_root: "", workshop_path: "" };

    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "auto_detect": {
          if (queue.length > 0) {
            lastAutoDetect = queue.shift() ?? lastAutoDetect;
          }
          return Promise.resolve(lastAutoDetect);
        }
        case "resolve_game_root":
          return Promise.resolve(gameRoot);
        case "check_optimizations":
          return Promise.resolve(optimizationsApplied);
        case "get_server_status":
          return Promise.resolve(serverStatus);
        case "open_workshop":
        case "play":
        case "append_launcher_log":
        case "write_launcher_log":
        case "open_launcher_log":
          return Promise.resolve();
        default:
          return Promise.resolve(undefined);
      }
    });
  };

  beforeEach(() => {
    invokeMock = invoke as Mock;
    invokeMock.mockReset();
    listenMock.mockClear();
    for (const key of Object.keys(listeners)) {
      delete listeners[key];
    }
  });

  it("shows download state and opens workshop on click", async () => {
    setupInvokeMock();

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("auto_detect", {
        workshopId: "3487726294",
      })
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("resolve_game_root"));

    invokeMock.mockClear();
    const bannerBtn = await screen.findByRole("button", { name: "Home Banner" });

    fireEvent.click(bannerBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("open_workshop", {
        workshopId: "3487726294",
      })
    );
  });

  it("auto-detects workshop and plays the game", async () => {
    setupInvokeMock({
      autoDetectQueue: [{ steam_root: "root", workshop_path: "wp" }],
      gameRoot: "game-root",
      optimizationsApplied: false,
    });

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("auto_detect", {
        workshopId: "3487726294",
      })
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("check_optimizations", {
        workshopPath: "wp",
      })
    );
    fireEvent.click(screen.getByRole("button", { name: "Play Tab" }));

    const playBtn = await screen.findByRole("button", { name: /^PLAY$/ });

    fireEvent.click(playBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("play", {
        appid: "108600",
        workshopId: "3487726294",
        workshopPath: "wp",
        extraArgs: null,
      })
    );

    await act(async () => {
      listeners["pz-session-ended"]?.({ payload: { cachedir: "wp", found: true } });
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^PLAY$/ })).toBeEnabled()
    );
  });

  it("refreshes detection and updates to ready state", async () => {
    setupInvokeMock({
      autoDetectQueue: [
        { steam_root: "", workshop_path: "" },
        { steam_root: "root", workshop_path: "wp" },
      ],
      gameRoot: "game-root",
      optimizationsApplied: false,
    });

    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Config Tab" }));
    const refreshBtn = await screen.findByRole("button", { name: "Refresh status" });
    fireEvent.click(refreshBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("auto_detect", {
        workshopId: "3487726294",
      })
    );

    await waitFor(() => expect(screen.getByText("ONLINE")).toBeInTheDocument());
  });

  it("switches to the settings tab", async () => {
    setupInvokeMock();

    render(<App />);

    const configTab = await screen.findByRole("button", { name: "Config Tab" });
    fireEvent.click(configTab);
    expect(
      await screen.findByText("Launcher configuration, folders, and diagnostics.")
    ).toBeInTheDocument();
  });
});
