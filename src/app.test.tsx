import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

describe("App", () => {
  let invokeMock: Mock;

  beforeEach(() => {
    invokeMock = invoke as Mock;
    invokeMock.mockReset();
    listenMock.mockClear();
    for (const key of Object.keys(listeners)) {
      delete listeners[key];
    }
  });

  it("shows download state and opens workshop on click", async () => {
    invokeMock
      .mockResolvedValueOnce({ steam_root: "", workshop_path: "" })
      .mockResolvedValueOnce("");

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("auto_detect", {
        workshopId: "3487726294",
      })
    );
    await waitFor(() => expect(screen.getByText(/Mod not found/i)).toBeInTheDocument());

    invokeMock.mockClear();
    const downloadBtn = screen.getByRole("button", { name: "Download" });

    invokeMock.mockResolvedValueOnce(undefined);
    fireEvent.click(downloadBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("open_workshop", {
        workshopId: "3487726294",
      })
    );
  });

  it("auto-detects workshop and plays the game", async () => {
    invokeMock
      .mockResolvedValueOnce({ steam_root: "root", workshop_path: "wp" })
      .mockResolvedValueOnce("game-root");

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("auto_detect", {
        workshopId: "3487726294",
      })
    );

    const playBtn = await screen.findByRole("button", { name: "Play" });

    invokeMock.mockClear();
    invokeMock.mockResolvedValueOnce(undefined);

    fireEvent.click(playBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("play", {
        appid: "108600",
        workshopId: "3487726294",
        workshopPath: "wp",
      })
    );

    listeners["pz-session-ended"]?.({ payload: { cachedir: "wp", found: true } });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Play" })).toBeEnabled()
    );
  });

  it("refreshes detection and updates to ready state", async () => {
    invokeMock
      .mockResolvedValueOnce({ steam_root: "", workshop_path: "" })
      .mockResolvedValueOnce("");

    render(<App />);

    const refreshBtn = await screen.findByRole("button", { name: "Refresh" });

    invokeMock
      .mockResolvedValueOnce({ steam_root: "root", workshop_path: "wp" })
      .mockResolvedValueOnce("game-root");

    fireEvent.click(refreshBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("auto_detect", {
        workshopId: "3487726294",
      })
    );

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument()
    );
  });

  it("toggles debug overlay", async () => {
    invokeMock
      .mockResolvedValueOnce({ steam_root: "", workshop_path: "" })
      .mockResolvedValueOnce("");

    render(<App />);

    const toggle = await screen.findByTitle("Show Settings & Console");
    fireEvent.click(toggle);
    expect(await screen.findByText("Steam root")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("Hide Settings & Console"));
    await waitFor(() => expect(screen.queryByText("Steam root")).toBeNull());
  });
});