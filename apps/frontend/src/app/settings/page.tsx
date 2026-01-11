'use client';

import { useQuery } from '@tanstack/react-query';
import { Loader2, Server, HardDrive, Wifi, Database, Activity } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsPanel } from '@/components/lcars/panel';
import { cn, formatBytes, formatDuration } from '@/lib/utils';

export default function SettingsPage() {
  // Fetch system status
  const { data: status, isLoading } = useQuery({
    queryKey: ['system', 'status'],
    queryFn: () => api.getSystemStatus(),
    refetchInterval: 10000, // Refresh every 10 seconds
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-12 w-12 animate-spin text-lcars-orange" />
      </div>
    );
  }

  if (!status) {
    return (
      <div className="space-y-6">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          Settings
        </h1>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-lg text-lcars-text-dim">Unable to load system information</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
        Settings
      </h1>

      {/* System Information */}
      <section>
        <h2 className="mb-4 flex items-center gap-2 text-xl font-bold uppercase tracking-wider text-lcars-blue">
          <Server className="h-6 w-6" />
          System Information
        </h2>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <LcarsPanel title="Application" accentColor="blue">
            <div className="space-y-3 text-sm">
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Version:</span>
                <span className="font-mono text-lcars-text">{status.version}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Uptime:</span>
                <span className="text-lcars-text">
                  {formatDuration(status.uptime_seconds)}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Database Size:</span>
                <span className="text-lcars-text">
                  {formatBytes(status.database_size_bytes)}
                </span>
              </div>
            </div>
          </LcarsPanel>

          <LcarsPanel title="Downloads" accentColor="orange">
            <div className="space-y-3 text-sm">
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Active:</span>
                <span className="text-lg font-bold text-lcars-orange">
                  {status.downloads.active}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Queued:</span>
                <span className="text-lg font-bold text-lcars-yellow">
                  {status.downloads.queued}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Seeding:</span>
                <span className="text-lg font-bold text-lcars-blue">
                  {status.downloads.seeding}
                </span>
              </div>
            </div>
          </LcarsPanel>
        </div>
      </section>

      {/* VPN Status */}
      <section>
        <h2 className="mb-4 flex items-center gap-2 text-xl font-bold uppercase tracking-wider text-lcars-purple">
          <Wifi className="h-6 w-6" />
          VPN Status
        </h2>
        <LcarsPanel
          accentColor={status.vpn.connected ? 'purple' : 'orange'}
          className="max-w-2xl"
        >
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-lcars-text-dim">Status:</span>
              <span
                className={cn(
                  'text-lg font-bold uppercase',
                  status.vpn.connected ? 'text-lcars-purple' : 'text-status-missing'
                )}
              >
                {status.vpn.connected ? 'Connected' : 'Disconnected'}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-lcars-text-dim">Enabled:</span>
              <span className="text-lcars-text">
                {status.vpn.enabled ? 'YES' : 'NO'}
              </span>
            </div>
            {status.vpn.interface && (
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Interface:</span>
                <span className="font-mono text-lcars-text">{status.vpn.interface}</span>
              </div>
            )}
            {status.vpn.public_ip && (
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">Public IP:</span>
                <span className="font-mono text-lcars-text">{status.vpn.public_ip}</span>
              </div>
            )}
          </div>
        </LcarsPanel>
      </section>

      {/* Storage Mounts */}
      <section>
        <h2 className="mb-4 flex items-center gap-2 text-xl font-bold uppercase tracking-wider text-lcars-yellow">
          <HardDrive className="h-6 w-6" />
          Storage Mounts
        </h2>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
          {status.storage.mounts.map((mount) => (
            <LcarsPanel
              key={mount.name}
              title={mount.name}
              accentColor={mount.available ? 'yellow' : 'orange'}
            >
              <div className="space-y-3 text-sm">
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Type:</span>
                  <span className="text-lcars-text uppercase">{mount.type}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Status:</span>
                  <span
                    className={cn(
                      'font-bold uppercase',
                      mount.available ? 'text-status-available' : 'text-status-missing'
                    )}
                  >
                    {mount.available ? 'Online' : 'Offline'}
                  </span>
                </div>
                {mount.available && mount.total_bytes && mount.free_bytes && (
                  <>
                    <div className="flex justify-between">
                      <span className="text-lcars-text-dim">Total:</span>
                      <span className="text-lcars-text">
                        {formatBytes(mount.total_bytes)}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-lcars-text-dim">Free:</span>
                      <span className="text-lcars-text">
                        {formatBytes(mount.free_bytes)}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-lcars-text-dim">Used:</span>
                      <span className="text-lcars-text">
                        {(
                          ((mount.total_bytes - mount.free_bytes) /
                            mount.total_bytes) *
                          100
                        ).toFixed(1)}
                        %
                      </span>
                    </div>
                  </>
                )}
                {mount.error && (
                  <div className="rounded bg-status-missing/20 p-2 text-xs text-status-missing">
                    {mount.error}
                  </div>
                )}
              </div>
            </LcarsPanel>
          ))}
        </div>
      </section>

      {/* Indexers Section - Placeholder */}
      <section>
        <h2 className="mb-4 flex items-center gap-2 text-xl font-bold uppercase tracking-wider text-lcars-orange">
          <Database className="h-6 w-6" />
          Indexers
        </h2>
        <LcarsPanel accentColor="orange" className="max-w-2xl">
          <div className="text-center text-sm text-lcars-text-dim">
            <p>Indexer configuration and management coming soon</p>
            <p className="mt-2 text-xs">
              This section will allow you to add, configure, and test indexers for finding releases
            </p>
          </div>
        </LcarsPanel>
      </section>

      {/* Activity Log Section - Placeholder */}
      <section>
        <h2 className="mb-4 flex items-center gap-2 text-xl font-bold uppercase tracking-wider text-lcars-blue">
          <Activity className="h-6 w-6" />
          Recent Activity
        </h2>
        <LcarsPanel accentColor="blue" className="max-w-2xl">
          <div className="text-center text-sm text-lcars-text-dim">
            <p>Activity log coming soon</p>
            <p className="mt-2 text-xs">
              This section will display recent system events, downloads, and media additions
            </p>
          </div>
        </LcarsPanel>
      </section>
    </div>
  );
}
