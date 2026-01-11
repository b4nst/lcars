import { forwardRef, HTMLAttributes } from 'react';
import { Pause, Play, X, RotateCw } from 'lucide-react';
import { cn, formatBytes, formatSpeed } from '@/lib/utils';
import { downloadStatusColors } from '@/lib/constants';
import { LcarsButton } from '@/components/lcars/button';
import type { Download } from '@/lib/types';

interface DownloadItemProps extends Omit<HTMLAttributes<HTMLDivElement>, 'onPause'> {
  download: Download;
  onPause?: (id: number) => void;
  onResume?: (id: number) => void;
  onCancel?: (id: number) => void;
  onRetry?: (id: number) => void;
}

export const DownloadItem = forwardRef<HTMLDivElement, DownloadItemProps>(
  ({ download, onPause, onResume, onCancel, onRetry, className, ...props }, ref) => {
    const canPause = download.status === 'downloading' || download.status === 'seeding';
    const canResume = download.status === 'paused';
    const canRetry = download.status === 'failed';
    const canCancel = !['completed', 'failed'].includes(download.status);

    return (
      <div
        ref={ref}
        className={cn(
          'rounded-lcars bg-lcars-dark p-4',
          className
        )}
        {...props}
      >
        {/* Header */}
        <div className="mb-3 flex items-start justify-between">
          <div className="flex-1 pr-4">
            <h3 className="text-sm font-bold text-lcars-text">
              {download.name}
            </h3>
            <div className="mt-1 flex items-center gap-2 text-xs text-lcars-text-dim">
              <span className="capitalize">{download.media_type}</span>
              <span>•</span>
              <span className={cn('capitalize', downloadStatusColors[download.status])}>
                {download.status}
              </span>
            </div>
          </div>

          {/* Action Buttons */}
          <div className="flex gap-2">
            {canPause && (
              <LcarsButton
                variant="yellow"
                size="sm"
                onClick={() => onPause?.(download.id)}
                title="Pause"
              >
                <Pause className="h-4 w-4" />
              </LcarsButton>
            )}
            {canResume && (
              <LcarsButton
                variant="orange"
                size="sm"
                onClick={() => onResume?.(download.id)}
                title="Resume"
              >
                <Play className="h-4 w-4" />
              </LcarsButton>
            )}
            {canRetry && (
              <LcarsButton
                variant="orange"
                size="sm"
                onClick={() => onRetry?.(download.id)}
                title="Retry"
              >
                <RotateCw className="h-4 w-4" />
              </LcarsButton>
            )}
            {canCancel && (
              <LcarsButton
                variant="red"
                size="sm"
                onClick={() => onCancel?.(download.id)}
                title="Cancel"
              >
                <X className="h-4 w-4" />
              </LcarsButton>
            )}
          </div>
        </div>

        {/* Progress Bar */}
        {download.status !== 'failed' && download.status !== 'completed' && (
          <div className="mb-3">
            <div className="h-2 w-full overflow-hidden rounded-full bg-lcars-black">
              <div
                className={cn(
                  'h-full transition-all duration-300',
                  downloadStatusColors[download.status]
                )}
                style={{ width: `${download.progress * 100}%` }}
              />
            </div>
            <div className="mt-1 text-xs text-lcars-text-dim">
              {(download.progress * 100).toFixed(1)}%
            </div>
          </div>
        )}

        {/* Stats */}
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-lcars-text-dim">
          {download.size_bytes && (
            <span>
              Size: {formatBytes(download.size_bytes)}
            </span>
          )}
          {download.download_speed > 0 && (
            <span>
              Down: {formatSpeed(download.download_speed)}
            </span>
          )}
          {download.upload_speed > 0 && (
            <span>
              Up: {formatSpeed(download.upload_speed)}
            </span>
          )}
          {download.peers > 0 && (
            <span>
              Peers: {download.peers}
            </span>
          )}
          {download.ratio > 0 && (
            <span>
              Ratio: {download.ratio.toFixed(2)}
            </span>
          )}
        </div>

        {/* Error Message */}
        {download.error_message && (
          <div className="mt-3 rounded-lcars bg-status-missing/20 p-2 text-xs text-status-missing">
            {download.error_message}
          </div>
        )}
      </div>
    );
  }
);

DownloadItem.displayName = 'DownloadItem';
