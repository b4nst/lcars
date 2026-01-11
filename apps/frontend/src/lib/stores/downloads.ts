import { create } from 'zustand';
import ReconnectingWebSocket from 'reconnecting-websocket';
import type { Download } from '../types';
import { useAuthStore } from './auth';

// WebSocket configuration constants
const WS_CONFIG = {
  maxReconnectionDelay: 10000,
  minReconnectionDelay: 1000,
  reconnectionDelayGrowFactor: 1.3,
  connectionTimeout: 4000,
  maxRetries: Infinity,
} as const;

interface DownloadsState {
  downloads: Map<number, Download>;
  ws: ReconnectingWebSocket | null;
  isConnected: boolean;
  connectionError: string | null;
  connect: () => void;
  disconnect: () => void;
  updateDownload: (download: Partial<Download> & { id: number }) => void;
  setDownloads: (downloads: Download[]) => void;
  getDownload: (id: number) => Download | undefined;
  removeDownload: (id: number) => void;
  clearOldDownloads: (maxAge?: number) => void;
}

export const useDownloadsStore = create<DownloadsState>((set, get) => ({
  downloads: new Map(),
  ws: null,
  isConnected: false,
  connectionError: null,

  connect: () => {
    const { ws: existingWs } = get();
    if (existingWs) {
      return;
    }

    // Determine WebSocket URL based on current location
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;

    // Include auth token in WebSocket connection if available
    const token = useAuthStore.getState().token;
    const wsUrl = token
      ? `${protocol}//${host}/api/ws?token=${encodeURIComponent(token)}`
      : `${protocol}//${host}/api/ws`;

    const ws = new ReconnectingWebSocket(wsUrl, [], {
      ...WS_CONFIG,
      debug: false,
    });

    ws.addEventListener('open', () => {
      set({ isConnected: true, connectionError: null });
    });

    ws.addEventListener('close', (event) => {
      const error = event.reason || null;
      set({ isConnected: false, connectionError: error });
    });

    ws.addEventListener('error', () => {
      set({ connectionError: 'WebSocket connection error' });
    });

    ws.addEventListener('message', (event) => {
      try {
        const message = JSON.parse(event.data);
        handleWebSocketMessage(message, get, set);
      } catch {
        // Silently ignore malformed messages in production
        if (process.env.NODE_ENV !== 'production') {
          console.error('Failed to parse WebSocket message');
        }
      }
    });

    set({ ws, isConnected: ws.readyState === WebSocket.OPEN });
  },

  disconnect: () => {
    const { ws } = get();
    if (ws) {
      console.log('Disconnecting WebSocket');
      ws.close();
      set({ ws: null, isConnected: false });
    }
  },

  updateDownload: (download) => {
    set((state) => {
      const downloads = new Map(state.downloads);
      const existing = downloads.get(download.id);
      if (existing) {
        downloads.set(download.id, { ...existing, ...download });
      } else {
        // If we don't have the full download object, we'll need to fetch it
        downloads.set(download.id, download as Download);
      }
      return { downloads };
    });
  },

  setDownloads: (downloadsList) => {
    const downloads = new Map(downloadsList.map((d) => [d.id, d]));
    set({ downloads });
  },

  getDownload: (id) => {
    return get().downloads.get(id);
  },

  removeDownload: (id) => {
    set((state) => {
      const downloads = new Map(state.downloads);
      downloads.delete(id);
      return { downloads };
    });
  },

  clearOldDownloads: (maxAge = 24 * 60 * 60 * 1000) => {
    // Default: clear completed downloads older than 24 hours
    const now = Date.now();
    set((state) => {
      const downloads = new Map(state.downloads);
      for (const [id, download] of downloads) {
        if (
          download.status === 'completed' &&
          download.completed_at &&
          now - new Date(download.completed_at).getTime() > maxAge
        ) {
          downloads.delete(id);
        }
      }
      return { downloads };
    });
  },
}));

// Handle WebSocket messages
function handleWebSocketMessage(
  message: any,
  get: () => DownloadsState,
  set: (partial: Partial<DownloadsState>) => void
) {
  const { type, ...data } = message;

  switch (type) {
    case 'download:added':
      if (data.download) {
        get().updateDownload(data.download);
      }
      break;

    case 'download:progress':
      get().updateDownload({
        id: data.id,
        progress: data.progress,
        download_speed: data.download_speed,
        upload_speed: data.upload_speed,
        peers: data.peers,
      });
      break;

    case 'download:status':
      get().updateDownload({
        id: data.id,
        status: data.status,
        error_message: data.error_message,
      });
      break;

    case 'download:completed':
      get().updateDownload({
        id: data.id,
        status: 'completed',
        progress: 1,
        completed_at: new Date().toISOString(),
      });
      break;

    case 'download:error':
      get().updateDownload({
        id: data.id,
        status: 'failed',
        error_message: data.error_message || 'Unknown error',
      });
      break;

    case 'media:added':
    case 'media:updated':
    case 'media:deleted':
    case 'system:status':
      // These can be handled by other stores or components if needed
      console.log('Received message:', type, data);
      break;

    default:
      console.warn('Unknown WebSocket message type:', type);
  }
}
