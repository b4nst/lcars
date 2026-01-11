'use client';

import { useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Loader2 } from 'lucide-react';
import { api } from '@/lib/api';
import { useDownloadsStore } from '@/lib/stores/downloads';
import { DownloadItem } from '@/components/download-item';
import { LcarsButton } from '@/components/lcars/button';
import { cn } from '@/lib/utils';
import type { Download, DownloadStatus } from '@/lib/types';

const STATUS_FILTERS: Array<{ label: string; value: DownloadStatus | 'all' }> = [
  { label: 'All', value: 'all' },
  { label: 'Downloading', value: 'downloading' },
  { label: 'Queued', value: 'queued' },
  { label: 'Seeding', value: 'seeding' },
  { label: 'Paused', value: 'paused' },
  { label: 'Completed', value: 'completed' },
  { label: 'Failed', value: 'failed' },
];

export default function DownloadsPage() {
  const queryClient = useQueryClient();

  // Get downloads from WebSocket store
  const wsDownloads = useDownloadsStore((state) => state.downloads);
  const setDownloads = useDownloadsStore((state) => state.setDownloads);

  // Fetch downloads from API (for initial load)
  const { data: apiDownloads, isLoading, isError, error } = useQuery({
    queryKey: ['downloads'],
    queryFn: () => api.getDownloads(),
  });

  // Sync API data with WebSocket store
  useEffect(() => {
    if (apiDownloads) {
      setDownloads(apiDownloads);
    }
  }, [apiDownloads, setDownloads]);

  // Merge WebSocket updates with API data
  const downloads = Array.from(wsDownloads.values());

  // Pause mutation
  const pauseMutation = useMutation({
    mutationFn: (id: number) => api.pauseDownload(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['downloads'] });
    },
  });

  // Resume mutation
  const resumeMutation = useMutation({
    mutationFn: (id: number) => api.resumeDownload(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['downloads'] });
    },
  });

  // Delete/Cancel mutation
  const deleteMutation = useMutation({
    mutationFn: (id: number) => api.deleteDownload(id, false),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['downloads'] });
    },
  });

  const handlePause = (id: number) => {
    pauseMutation.mutate(id);
  };

  const handleResume = (id: number) => {
    resumeMutation.mutate(id);
  };

  const handleCancel = (id: number) => {
    deleteMutation.mutate(id);
  };

  const handleRetry = (id: number) => {
    // Retry is the same as resume for failed downloads
    resumeMutation.mutate(id);
  };

  // Group downloads by status
  const activeDownloads = downloads.filter((d) => d.status === 'downloading');
  const queuedDownloads = downloads.filter((d) => d.status === 'queued');
  const seedingDownloads = downloads.filter((d) => d.status === 'seeding');
  const pausedDownloads = downloads.filter((d) => d.status === 'paused');
  const completedDownloads = downloads.filter((d) => d.status === 'completed');
  const failedDownloads = downloads.filter((d) => d.status === 'failed');

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-12 w-12 animate-spin text-lcars-orange" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-6">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          Downloads
        </h1>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-status-missing">
            Failed to load downloads: {error?.message || 'Unknown error'}
          </p>
        </div>
      </div>
    );
  }

  if (downloads.length === 0) {
    return (
      <div className="space-y-6">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          Downloads
        </h1>
        <div className="flex flex-col items-center justify-center rounded-lcars bg-lcars-dark py-12">
          <p className="text-lg text-lcars-text-dim">No downloads</p>
          <p className="mt-2 text-sm text-lcars-text-dim">
            Start downloading media from Movies, TV Shows, or Music sections
          </p>
        </div>
      </div>
    );
  }

  const sectionColors: Record<string, string> = {
    Active: 'text-lcars-orange',
    Queued: 'text-lcars-yellow',
    Seeding: 'text-lcars-blue',
    Paused: 'text-lcars-tan',
    Failed: 'text-status-missing',
    Completed: 'text-status-available',
  };

  const renderDownloadSection = (
    title: string,
    downloads: Download[]
  ) => {
    if (downloads.length === 0) return null;

    return (
      <div>
        <h2
          className={cn(
            'mb-4 text-xl font-bold uppercase tracking-wider',
            sectionColors[title] || 'text-lcars-orange'
          )}
        >
          {title} ({downloads.length})
        </h2>
        <div className="space-y-3">
          {downloads.map((download) => (
            <DownloadItem
              key={download.id}
              download={download}
              onPause={handlePause}
              onResume={handleResume}
              onCancel={handleCancel}
              onRetry={handleRetry}
            />
          ))}
        </div>
      </div>
    );
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          Downloads
        </h1>
        <div className="text-sm text-lcars-text-dim">
          {downloads.length} {downloads.length === 1 ? 'download' : 'downloads'}
        </div>
      </div>

      {/* Active Downloads */}
      {renderDownloadSection('Active', activeDownloads)}

      {/* Queued Downloads */}
      {renderDownloadSection('Queued', queuedDownloads)}

      {/* Seeding Downloads */}
      {renderDownloadSection('Seeding', seedingDownloads)}

      {/* Paused Downloads */}
      {renderDownloadSection('Paused', pausedDownloads)}

      {/* Failed Downloads */}
      {renderDownloadSection('Failed', failedDownloads)}

      {/* Completed Downloads */}
      {renderDownloadSection('Completed', completedDownloads)}
    </div>
  );
}
