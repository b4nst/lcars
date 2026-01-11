import { forwardRef, HTMLAttributes } from 'react';
import Image from 'next/image';
import { Music } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Artist } from '@/lib/types';

interface ArtistCardProps extends HTMLAttributes<HTMLDivElement> {
  artist: Artist;
  onSelect?: () => void;
}

export const ArtistCard = forwardRef<HTMLDivElement, ArtistCardProps>(
  ({ artist, onSelect, className, ...props }, ref) => {
    const imageUrl = artist.image_path || null;
    const albumCount = artist.albums?.length || 0;

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
          onSelect && 'cursor-pointer hover:ring-2 hover:ring-lcars-purple hover:scale-105 focus:outline-none focus:ring-2 focus:ring-lcars-purple',
          className
        )}
        onClick={onSelect}
        onKeyDown={handleKeyDown}
        {...props}
      >
        {/* Artist Image - Square Aspect Ratio */}
        <div className="relative aspect-square w-full overflow-hidden bg-lcars-black">
          {imageUrl ? (
            <Image
              src={imageUrl}
              alt={artist.name}
              fill
              className="object-cover transition-opacity duration-300 group-hover:opacity-80"
              sizes="(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 20vw"
            />
          ) : (
            <div className="flex h-full w-full items-center justify-center bg-lcars-dark">
              <Music className="h-16 w-16 text-lcars-purple opacity-30" />
            </div>
          )}

          {/* Monitored Badge */}
          {artist.monitored && (
            <div className="absolute top-2 right-2">
              <div className="rounded-lcars bg-lcars-purple px-2 py-1 text-xs font-bold text-lcars-black">
                MONITORED
              </div>
            </div>
          )}
        </div>

        {/* Info Overlay */}
        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-lcars-black via-lcars-black/80 to-transparent p-3">
          <h3 className="truncate text-sm font-bold text-lcars-text">
            {artist.name}
          </h3>
          <div className="mt-1 flex items-center justify-between text-xs text-lcars-text-dim">
            <span>{albumCount} {albumCount === 1 ? 'album' : 'albums'}</span>
          </div>
          {artist.disambiguation && (
            <p className="mt-1 truncate text-xs text-lcars-text-dim">
              {artist.disambiguation}
            </p>
          )}
        </div>
      </div>
    );
  }
);

ArtistCard.displayName = 'ArtistCard';
