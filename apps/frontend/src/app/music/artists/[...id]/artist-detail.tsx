'use client';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import Image from 'next/image';
import { ArrowLeft, Trash2, Loader2, Music } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import { LcarsPanel } from '@/components/lcars/panel';
import { cn } from '@/lib/utils';
import { mediaStatusColors } from '@/lib/constants';
import type { AlbumStatus } from '@/lib/types';

interface ArtistDetailProps {
  id: string;
}

export function ArtistDetail({ id }: ArtistDetailProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const artistId = parseInt(id, 10);

  // Fetch artist details
  const { data: artist, isLoading, isError, error } = useQuery({
    queryKey: ['artists', artistId],
    queryFn: () => api.getArtist(artistId),
  });

  // Delete mutation
  const deleteMutation = useMutation({
    mutationFn: () => api.deleteArtist(artistId, false),
    onSuccess: () => {
      router.push('/music');
    },
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-12 w-12 animate-spin text-lcars-purple" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-4">
        <LcarsButton variant="purple" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-status-missing">
            Failed to load artist: {error?.message || 'Unknown error'}
          </p>
        </div>
      </div>
    );
  }

  if (!artist) {
    return (
      <div className="space-y-4">
        <LcarsButton variant="purple" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-lg text-lcars-text-dim">Artist not found</p>
        </div>
      </div>
    );
  }

  const getStatusColor = (status: AlbumStatus) => {
    if (status === 'available') return 'text-status-available';
    if (status === 'partial') return 'text-lcars-yellow';
    if (status === 'downloading') return 'text-status-downloading';
    return 'text-status-missing';
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <LcarsButton variant="purple" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <LcarsButton variant="red" onClick={() => deleteMutation.mutate()}>
          <Trash2 className="mr-2 h-5 w-5" />
          Delete
        </LcarsButton>
      </div>

      {/* Artist Info */}
      <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
        {/* Artist Image */}
        <div className="md:col-span-1">
          <div className="relative aspect-square overflow-hidden rounded-lcars bg-lcars-black">
            {artist.image_path ? (
              <Image
                src={artist.image_path}
                alt={artist.name}
                fill
                className="object-cover"
                priority
                sizes="(max-width: 768px) 100vw, 33vw"
              />
            ) : (
              <div className="flex h-full w-full items-center justify-center">
                <Music className="h-32 w-32 text-lcars-purple opacity-30" />
              </div>
            )}
          </div>
        </div>

        {/* Details */}
        <div className="space-y-4 md:col-span-2">
          <div>
            <h1 className="text-3xl font-bold uppercase tracking-wider text-lcars-purple">
              {artist.name}
            </h1>
            {artist.sort_name && artist.sort_name !== artist.name && (
              <p className="mt-1 text-lg text-lcars-text-dim">{artist.sort_name}</p>
            )}
            {artist.disambiguation && (
              <p className="mt-1 text-sm text-lcars-text-dim">({artist.disambiguation})</p>
            )}
          </div>

          {/* Status Panel */}
          <LcarsPanel accentColor="purple">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Monitored:</span>
                <span className="text-sm font-bold text-lcars-text">
                  {artist.monitored ? 'YES' : 'NO'}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Albums:</span>
                <span className="text-sm font-bold text-lcars-text">
                  {artist.albums.length}
                </span>
              </div>
              {artist.country && (
                <div className="flex items-center justify-between">
                  <span className="text-sm text-lcars-text-dim">Country:</span>
                  <span className="text-sm font-bold text-lcars-text">
                    {artist.country}
                  </span>
                </div>
              )}
            </div>
          </LcarsPanel>

          {/* Bio/Overview */}
          {artist.overview && (
            <LcarsPanel title="Biography" accentColor="blue">
              <p className="text-sm leading-relaxed text-lcars-text">{artist.overview}</p>
            </LcarsPanel>
          )}

          {/* Metadata */}
          <LcarsPanel title="Details" accentColor="purple">
            <div className="space-y-2 text-sm">
              {artist.artist_type && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Type:</span>
                  <span className="text-lcars-text capitalize">{artist.artist_type}</span>
                </div>
              )}
              {artist.begin_date && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Active:</span>
                  <span className="text-lcars-text">
                    {artist.begin_date}
                    {artist.end_date ? ` - ${artist.end_date}` : ' - Present'}
                  </span>
                </div>
              )}
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">MusicBrainz ID:</span>
                <span className="text-lcars-text font-mono text-xs">{artist.mbid}</span>
              </div>
            </div>
          </LcarsPanel>
        </div>
      </div>

      {/* Albums */}
      <div>
        <h2 className="mb-4 text-xl font-bold uppercase tracking-wider text-lcars-purple">
          Albums ({artist.albums.length})
        </h2>
        {artist.albums.length > 0 ? (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {artist.albums.map((album) => (
              <Link key={album.id} href={`/music/albums/${album.id}`}>
                <div className="group cursor-pointer rounded-lcars bg-lcars-dark p-4 transition-all hover:ring-2 hover:ring-lcars-purple">
                  <div className="flex items-start gap-3">
                    {/* Album Cover Thumbnail */}
                    <div className="relative h-16 w-16 shrink-0 overflow-hidden rounded bg-lcars-black">
                      {album.cover_path ? (
                        <Image
                          src={album.cover_path}
                          alt={album.title}
                          fill
                          className="object-cover"
                          sizes="64px"
                        />
                      ) : (
                        <div className="flex h-full w-full items-center justify-center">
                          <Music className="h-8 w-8 text-lcars-purple opacity-30" />
                        </div>
                      )}
                    </div>

                    {/* Album Info */}
                    <div className="flex-1 min-w-0">
                      <h3 className="truncate text-sm font-bold text-lcars-text">
                        {album.title}
                      </h3>
                      <div className="mt-1 flex items-center gap-2">
                        <span
                          className={cn(
                            'text-xs font-bold uppercase',
                            getStatusColor(album.status)
                          )}
                        >
                          {album.status}
                        </span>
                        {album.release_date && (
                          <>
                            <span className="text-xs text-lcars-text-dim">•</span>
                            <span className="text-xs text-lcars-text-dim">
                              {new Date(album.release_date).getFullYear()}
                            </span>
                          </>
                        )}
                      </div>
                      {album.total_tracks && (
                        <p className="mt-1 text-xs text-lcars-text-dim">
                          {album.total_tracks}{' '}
                          {album.total_tracks === 1 ? 'track' : 'tracks'}
                        </p>
                      )}
                    </div>
                  </div>
                </div>
              </Link>
            ))}
          </div>
        ) : (
          <div className="rounded-lcars bg-lcars-dark p-8 text-center">
            <p className="text-lcars-text-dim">No albums available</p>
          </div>
        )}
      </div>
    </div>
  );
}
