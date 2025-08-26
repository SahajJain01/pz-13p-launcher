import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from './app';
import { invoke } from '@tauri-apps/api/core';
import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('App', () => {
  let invokeMock: Mock;

  beforeEach(() => {
    invokeMock = invoke as Mock;
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it('shows download state and opens workshop on click', async () => {
    invokeMock.mockResolvedValueOnce({ steam_root: '', workshop_path: '', mods_path: '' });

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('auto_detect', { workshopId: '3487726294' })
    );

    expect(screen.getByText(/Mod not found/i)).toBeInTheDocument();

    invokeMock.mockClear();

    const downloadBtn = screen.getByRole('button', { name: 'Download' });
    fireEvent.click(downloadBtn);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('open_workshop', { workshopId: '3487726294' })
    );
  });

  it('auto-detects workshop and plays the game', async () => {
    invokeMock
      .mockResolvedValueOnce({ steam_root: '', workshop_path: 'wp', mods_path: 'mods' })
      .mockResolvedValueOnce({});

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('auto_detect', { workshopId: '3487726294' })
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('cleanup', { modsPath: 'mods' })
    );

    invokeMock.mockClear();
    invokeMock
      .mockResolvedValueOnce({ linked: 1, backups: 0 })
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce({});

    const playBtn = screen.getByRole('button', { name: 'Play' });
    fireEvent.click(playBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('link_all', { workshopPath: 'wp', modsPath: 'mods' })
    );
    expect(invokeMock).toHaveBeenCalledWith('play', { appid: '108600' });
    expect(invokeMock).toHaveBeenCalledWith('cleanup', { modsPath: 'mods' });

    await waitFor(() => expect(screen.getByRole('button', { name: 'Play' })).toBeEnabled());
  });

  it('refreshes detection and updates to ready state', async () => {
    invokeMock
      .mockResolvedValueOnce({ steam_root: '', workshop_path: '', mods_path: '' })
      .mockResolvedValueOnce({ steam_root: '', workshop_path: 'wp', mods_path: 'mods' })
      .mockResolvedValueOnce({});

    render(<App />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('auto_detect', { workshopId: '3487726294' })
    );

    const refreshBtn = await screen.findByRole('button', { name: 'Refresh' });
    invokeMock.mockClear();
    fireEvent.click(refreshBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('auto_detect', { workshopId: '3487726294' })
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('cleanup', { modsPath: 'mods' })
    );
    await waitFor(() => expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument());
  });

  it('toggles debug overlay', async () => {
    invokeMock.mockResolvedValueOnce({ steam_root: '', workshop_path: '', mods_path: '' });

    render(<App />);

    await waitFor(() => expect(invokeMock).toHaveBeenCalled());

    const toggle = screen.getByTitle('Show Debug Info');
    fireEvent.click(toggle);
    expect(await screen.findByText('Steam root')).toBeInTheDocument();
    fireEvent.click(toggle);
    await waitFor(() => expect(screen.queryByText('Steam root')).toBeNull());
  });
});

