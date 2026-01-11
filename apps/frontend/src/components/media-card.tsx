import { forwardRef, HTMLAttributes } from 'react';
import Image from 'next/image';
import { cn } from '@/lib/utils';
import { mediaStatusColors } from '@/lib/constants';
import type { Movie, TvShow } from '@/lib/types';

interface MediaCardProps extends HTMLAttributes<HTMLDivElement> {
  media: Movie | TvShow;
  onSelect?: () => void;
}

export const MediaCard = forwardRef<HTMLDivElement, MediaCardProps>(
  ({ media, onSelect, className, ...props }, ref) => {
    const posterUrl = media.poster_path
      ? `https://image.tmdb.org/t/p/w500${media.poster_path}`
      : '/placeholder-poster.png';

    const year = 'year' in media ? media.year : media.year_start;

    // For movies, show the status indicator; for TV shows, we'll use a different approach
    const isMovie = 'year' in media;
    const mediaStatus = isMovie ? (media as Movie).status : null;

    const handleKeyDown = (e: React.KeyboardEvent) => {
      if ((e.key === 'Enter' || e.key === ' ') && onSelect) {
        e.preventDefault();
        onSelect();
      }
    };

    return (
      <div
        ref={ref}
        role={onSelect ? 'button' : undefined}
        tabIndex={onSelect ? 0 : undefined}
        className={cn(
          'group relative overflow-hidden rounded-lcars',
          'bg-lcars-dark transition-all duration-300',
          onSelect && 'cursor-pointer hover:ring-2 hover:ring-lcars-orange hover:scale-105 focus:outline-none focus:ring-2 focus:ring-lcars-orange',
          className
        )}
        onClick={onSelect}
        onKeyDown={handleKeyDown}
        {...props}
      >
        {/* Poster Image */}
        <div className="relative aspect-[2/3] w-full overflow-hidden bg-lcars-black">
          <Image
            src={posterUrl}
            alt={media.title}
            fill
            className="object-cover transition-opacity duration-300 group-hover:opacity-80"
            sizes="(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 20vw"
          />

          {/* Status Indicator - Only for Movies */}
          {mediaStatus && (
            <div className="absolute top-2 left-2">
              <div
                className={cn(
                  'h-3 w-3 rounded-full',
                  mediaStatusColors[mediaStatus]
                )}
                title={mediaStatus}
              />
            </div>
          )}

          {/* Monitored Badge */}
          {media.monitored && (
            <div className="absolute top-2 right-2">
              <div className="rounded-lcars bg-lcars-orange px-2 py-1 text-xs font-bold text-lcars-black">
                MONITORED
              </div>
            </div>
          )}
        </div>

        {/* Info Overlay */}
        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-lcars-black via-lcars-black/80 to-transparent p-3">
          <h3 className="truncate text-sm font-bold text-lcars-text">
            {media.title}
          </h3>
          {year && (
            <p className="text-xs text-lcars-text-dim">{year}</p>
          )}
        </div>
      </div>
    );
  }
);

MediaCard.displayName = 'MediaCard';
