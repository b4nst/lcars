'use client';

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import Image from 'next/image';
import { ArrowLeft, Search, Download, Trash2, Loader2, Music } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import { LcarsPanel } from '@/components/lcars/panel';
import { TrackList } from '@/components/track-list';
import { cn, formatBytes } from '@/lib/utils';
import type { Release } from '@/lib/types';

interface AlbumDetailProps {
  id: string;
}

export function AlbumDetail({ id }: AlbumDetailProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const albumId = parseInt(id, 10);
  const [releases, setReleases] = useState<Release[]>([]);
  const [isSearching, setIsSearching] = useState(false);

  // Fetch album details
  const { data: album, isLoading, isError, error } = useQuery({
    queryKey: ['albums', albumId],
    queryFn: () => api.getAlbum(albumId),
  });

  // Search releases mutation
  const searchMutation = useMutation({
    mutationFn: () => api.searchAlbumReleases(albumId),
    onSuccess: (data) => {
      setReleases(data);
      setIsSearching(false);
    },
    onError: () => {
      setIsSearching(false);
    },
  });

  // Download mutation
  const downloadMutation = useMutation({
    mutationFn: (releaseId: string) =>
      api.downloadAlbum(albumId, { release_id: releaseId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['albums', albumId] });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    mutationFn: () => api.deleteAlbum(albumId, false),
    onSuccess: () => {
      router.back();
    },
  });

  const handleSearchReleases = () => {
    setIsSearching(true);
    searchMutation.mutate();
  };

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
            Failed to load album: {error?.message || 'Unknown error'}
          </p>
        </div>
      </div>
    );
  }

  if (!album) {
    return (
      <div className="space-y-4">
        <LcarsButton variant="purple" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-lg text-lcars-text-dim">Album not found</p>
        </div>
      </div>
    );
  }

  const getStatusColor = () => {
    if (album.status === 'available') return 'text-status-available';
    if (album.status === 'partial') return 'text-lcars-yellow';
    if (album.status === 'downloading') return 'text-status-downloading';
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

      {/* Album Info */}
      <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
        {/* Album Cover */}
        <div className="md:col-span-1">
          <div className="relative aspect-square overflow-hidden rounded-lcars bg-lcars-black">
            {album.cover_path ? (
              <Image
                src={album.cover_path}
                alt={album.title}
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
              {album.title}
            </h1>
            {album.release_date && (
              <p className="mt-1 text-lg text-lcars-text-dim">
                {new Date(album.release_date).getFullYear()}
              </p>
            )}
          </div>

          {/* Status Panel */}
          <LcarsPanel accentColor="purple">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Status:</span>
                <span className={cn('text-sm font-bold uppercase', getStatusColor())}>
                  {album.status}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Monitored:</span>
                <span className="text-sm font-bold text-lcars-text">
                  {album.monitored ? 'YES' : 'NO'}
                </span>
              </div>
              {album.total_tracks && (
                <div className="flex items-center justify-between">
                  <span className="text-sm text-lcars-text-dim">Tracks:</span>
                  <span className="text-sm font-bold text-lcars-text">
                    {album.total_tracks}
                  </span>
                </div>
              )}
            </div>
          </LcarsPanel>

          {/* Overview */}
          {album.overview && (
            <LcarsPanel title="About" accentColor="blue">
              <p className="text-sm leading-relaxed text-lcars-text">{album.overview}</p>
            </LcarsPanel>
          )}

          {/* Metadata */}
          <LcarsPanel title="Details" accentColor="purple">
            <div className="space-y-2 text-sm">
              {album.album_type && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Type:</span>
                  <span className="text-lcars-text capitalize">{album.album_type}</span>
                </div>
              )}
              {album.release_date && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Release Date:</span>
                  <span className="text-lcars-text">
                    {new Date(album.release_date).toLocaleDateString()}
                  </span>
                </div>
              )}
              <div className="flex justify-between">
                <span className="text-lcars-text-dim">MusicBrainz ID:</span>
                <span className="text-lcars-text font-mono text-xs">{album.mbid}</span>
              </div>
            </div>
          </LcarsPanel>
        </div>
      </div>

      {/* Tracks */}
      <div>
        <h2 className="mb-4 text-xl font-bold uppercase tracking-wider text-lcars-purple">
          Tracks
        </h2>
        {album.tracks.length > 0 ? (
          <TrackList tracks={album.tracks} />
        ) : (
          <div className="rounded-lcars bg-lcars-dark p-8 text-center">
            <p className="text-lcars-text-dim">No tracks available</p>
          </div>
        )}
      </div>

      {/* Search Releases */}
      <div>
        <LcarsButton
          variant="purple"
          onClick={handleSearchReleases}
          disabled={isSearching}
        >
          {isSearching ? (
            <>
              <Loader2 className="mr-2 h-5 w-5 animate-spin" />
              Searching...
            </>
          ) : (
            <>
              <Search className="mr-2 h-5 w-5" />
              Search Releases
            </>
          )}
        </LcarsButton>
      </div>

      {/* Releases List */}
      {releases.length > 0 && (
        <div>
          <h2 className="mb-4 text-xl font-bold uppercase tracking-wider text-lcars-purple">
            Available Releases ({releases.length})
          </h2>
          <div className="space-y-3">
            {releases.map((release) => (
              <div key={release.id} className="rounded-lcars bg-lcars-dark p-4">
                <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex-1">
                    <h3 className="text-sm font-bold text-lcars-text">{release.title}</h3>
                    <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-lcars-text-dim">
                      <span>Size: {formatBytes(release.size_bytes)}</span>
                      <span>Quality: {release.quality}</span>
                      <span>Source: {release.source}</span>
                      <span>Seeders: {release.seeders}</span>
                      {release.group && <span>Group: {release.group}</span>}
                    </div>
                  </div>
                  <LcarsButton
                    variant="purple"
                    size="sm"
                    onClick={() => downloadMutation.mutate(release.id)}
                    disabled={downloadMutation.isPending}
                  >
                    <Download className="mr-2 h-4 w-4" />
                    Download
                  </LcarsButton>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
