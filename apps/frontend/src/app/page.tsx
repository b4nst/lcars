'use client';

import { useQuery } from '@tanstack/react-query';
import Link from 'next/link';
import { ArrowRight, Activity, Download, HardDrive, Wifi } from 'lucide-react';
import { api } from '@/lib/api';
import { useDownloadsStore } from '@/lib/stores/downloads';
import { LcarsPanel } from '@/components/lcars/panel';
import { LcarsButton } from '@/components/lcars/button';
import { MediaGrid } from '@/components/media-grid';
import { MediaCard } from '@/components/media-card';
import { DownloadItem } from '@/components/download-item';
import { cn, formatDuration } from '@/lib/utils';

export default function Dashboard() {
  // Fetch system status with refetch interval for real-time updates
  const { data: status } = useQuery({
    queryKey: ['system', 'status'],
    queryFn: () => api.getSystemStatus(),
    refetchInterval: 5000, // Refresh every 5 seconds
  });

  // Fetch recent movies
  const { data: moviesResponse } = useQuery({
    queryKey: ['movies', 'recent'],
    queryFn: () => api.getMovies({ limit: 6, page: 1 }),
  });

  // Get active downloads from WebSocket store
  const downloads = useDownloadsStore((state) => Array.from(state.downloads.values()));
  const activeDownloads = downloads
    .filter((d) => ['downloading', 'queued', 'seeding'].includes(d.status))
    .slice(0, 5);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          Dashboard
        </h1>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {/* Active Downloads */}
        <LcarsPanel accentColor="orange">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs text-lcars-text-dim">ACTIVE DOWNLOADS</p>
              <p className="mt-1 text-3xl font-bold text-lcars-orange">
                {status?.downloads.active || 0}
              </p>
            </div>
            <Download className="h-10 w-10 text-lcars-orange opacity-50" />
          </div>
        </LcarsPanel>

        {/* Queued */}
        <LcarsPanel accentColor="yellow">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs text-lcars-text-dim">QUEUED</p>
              <p className="mt-1 text-3xl font-bold text-lcars-yellow">
                {status?.downloads.queued || 0}
              </p>
            </div>
            <Activity className="h-10 w-10 text-lcars-yellow opacity-50" />
          </div>
        </LcarsPanel>

        {/* Seeding */}
        <LcarsPanel accentColor="blue">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs text-lcars-text-dim">SEEDING</p>
              <p className="mt-1 text-3xl font-bold text-lcars-blue">
                {status?.downloads.seeding || 0}
              </p>
            </div>
            <HardDrive className="h-10 w-10 text-lcars-blue opacity-50" />
          </div>
        </LcarsPanel>

        {/* VPN Status */}
        <LcarsPanel accentColor={status?.vpn.connected ? 'purple' : 'orange'}>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs text-lcars-text-dim">VPN STATUS</p>
              <p
                className={cn(
                  'mt-1 text-lg font-bold uppercase',
                  status?.vpn.connected ? 'text-lcars-purple' : 'text-status-missing'
                )}
              >
                {status?.vpn.connected ? 'Connected' : 'Disconnected'}
              </p>
              {status?.vpn.interface && (
                <p className="mt-1 text-xs text-lcars-text-dim">
                  {status.vpn.interface}
                </p>
              )}
            </div>
            <Wifi
              className={cn(
                'h-10 w-10 opacity-50',
                status?.vpn.connected ? 'text-lcars-purple' : 'text-status-missing'
              )}
            />
          </div>
        </LcarsPanel>
      </div>

      {/* System Info */}
      {status && (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <LcarsPanel title="System" accentColor="blue">
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Version:</span>
                <span className="text-lcars-text">{status.version}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Uptime:</span>
                <span className="text-lcars-text">
                  {formatDuration(status.uptime_seconds)}
                </span>
              </div>
            </div>
          </LcarsPanel>

          {status.storage?.mounts && status.storage.mounts.length > 0 && (
            <LcarsPanel title="Storage" accentColor="purple">
              <div className="space-y-2 text-sm">
                {status.storage.mounts.map((mount) => (
                  <div key={mount.name} className="flex justify-between">
                    <span className="text-lcars-text-dim">{mount.name}:</span>
                    <span
                      className={cn(
                        'uppercase',
                        mount.available ? 'text-status-available' : 'text-status-missing'
                      )}
                    >
                      {mount.available ? 'Online' : 'Offline'}
                    </span>
                  </div>
                ))}
              </div>
            </LcarsPanel>
          )}
        </div>
      )}

      {/* Active Downloads */}
      {activeDownloads.length > 0 && (
        <div>
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-bold uppercase tracking-wider text-lcars-orange">
              Active Downloads
            </h2>
            <Link href="/downloads">
              <LcarsButton variant="orange" size="sm">
                View All
                <ArrowRight className="ml-2 h-4 w-4" />
              </LcarsButton>
            </Link>
          </div>
          <div className="space-y-3">
            {activeDownloads.map((download) => (
              <DownloadItem key={download.id} download={download} />
            ))}
          </div>
        </div>
      )}

      {/* Recent Movies */}
      {moviesResponse && moviesResponse.items.length > 0 && (
        <div>
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-xl font-bold uppercase tracking-wider text-lcars-orange">
              Recent Movies
            </h2>
            <Link href="/movies">
              <LcarsButton variant="orange" size="sm">
                View All
                <ArrowRight className="ml-2 h-4 w-4" />
              </LcarsButton>
            </Link>
          </div>
          <MediaGrid>
            {moviesResponse.items.map((movie) => (
              <Link key={movie.id} href={`/movies/${movie.id}`}>
                <MediaCard media={movie} />
              </Link>
            ))}
          </MediaGrid>
        </div>
      )}
    </div>
  );
}
