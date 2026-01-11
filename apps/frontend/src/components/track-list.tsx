import { forwardRef, HTMLAttributes, useMemo } from 'react';
import { cn } from '@/lib/utils';
import { mediaStatusColors } from '@/lib/constants';
import type { Track } from '@/lib/types';

interface TrackListProps extends HTMLAttributes<HTMLDivElement> {
  tracks: Track[];
  onTrackClick?: (track: Track) => void;
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}`;
}

export const TrackList = forwardRef<HTMLDivElement, TrackListProps>(
  ({ tracks, onTrackClick, className, ...props }, ref) => {
    // Group tracks by disc number - memoized to avoid recalculation
    const { tracksByDisc, discs } = useMemo(() => {
      const grouped = tracks.reduce((acc, track) => {
        const disc = track.disc_number;
        if (!acc[disc]) {
          acc[disc] = [];
        }
        acc[disc].push(track);
        return acc;
      }, {} as Record<number, Track[]>);

      const sortedDiscs = Object.keys(grouped)
        .map(Number)
        .sort((a, b) => a - b);

      return { tracksByDisc: grouped, discs: sortedDiscs };
    }, [tracks]);

    // Empty state
    if (tracks.length === 0) {
      return (
        <div ref={ref} className={cn('space-y-4', className)} {...props}>
          <div className="py-8 text-center text-sm text-lcars-text-dim">
            No tracks available
          </div>
        </div>
      );
    }

    return (
      <div ref={ref} className={cn('space-y-4', className)} {...props}>
        {discs.map((discNumber) => {
          const discTracks = tracksByDisc[discNumber].sort(
            (a, b) => a.track_number - b.track_number
          );

          return (
            <div key={discNumber} className="space-y-1">
              {/* Disc Header (only show if multiple discs) */}
              {discs.length > 1 && (
                <div className="mb-2 border-b border-lcars-orange pb-1">
                  <h3 className="text-sm font-bold text-lcars-orange">
                    DISC {discNumber}
                  </h3>
                </div>
              )}

              {/* Track List */}
              {discTracks.map((track) => (
                <div
                  key={track.id}
                  className={cn(
                    'group flex items-center justify-between rounded-lcars px-3 py-2',
                    'bg-lcars-dark transition-all duration-200',
                    onTrackClick && 'cursor-pointer hover:bg-lcars-orange/20 hover:ring-1 hover:ring-lcars-orange'
                  )}
                  onClick={() => onTrackClick?.(track)}
                >
                  <div className="flex flex-1 items-center gap-3">
                    {/* Status Indicator */}
                    <div
                      className={cn(
                        'h-2 w-2 shrink-0 rounded-full',
                        mediaStatusColors[track.status]
                      )}
                      title={track.status}
                    />

                    {/* Track Number */}
                    <span className="w-8 shrink-0 text-sm text-lcars-text-dim">
                      {track.track_number}
                    </span>

                    {/* Track Title */}
                    <span className="flex-1 truncate text-sm text-lcars-text">
                      {track.title}
                    </span>
                  </div>

                  {/* Duration */}
                  {track.duration_ms && (
                    <span className="shrink-0 text-sm text-lcars-text-dim">
                      {formatDuration(track.duration_ms)}
                    </span>
                  )}
                </div>
              ))}
            </div>
          );
        })}
      </div>
    );
  }
);

TrackList.displayName = 'TrackList';
