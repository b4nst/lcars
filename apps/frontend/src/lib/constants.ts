import type { MediaStatus, DownloadStatus } from '@/lib/types';

/**
 * Color mappings for media status indicators
 */
export const mediaStatusColors: Record<MediaStatus, string> = {
  available: 'bg-status-available',
  downloading: 'bg-status-downloading',
  processing: 'bg-status-processing',
  missing: 'bg-status-missing',
  searching: 'bg-lcars-yellow',
};

/**
 * Color mappings for download status indicators
 */
export const downloadStatusColors: Record<DownloadStatus, string> = {
  queued: 'bg-lcars-yellow',
  downloading: 'bg-status-downloading',
  seeding: 'bg-status-available',
  processing: 'bg-status-processing',
  completed: 'bg-status-available',
  failed: 'bg-status-missing',
  paused: 'bg-lcars-tan',
};
