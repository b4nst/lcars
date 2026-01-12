'use client';

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import Image from 'next/image';
import { ArrowLeft, Search, Download, Trash2, Loader2 } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import { LcarsPanel } from '@/components/lcars/panel';
import { cn, formatBytes } from '@/lib/utils';
import { mediaStatusColors } from '@/lib/constants';
import type { Release } from '@/lib/types';

// Required for static export - pages will be client-rendered
export function generateStaticParams() {
  return [];
}

interface MovieDetailPageProps {
  params: {
    id: string;
  };
}

export default function MovieDetailPage({ params }: MovieDetailPageProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const movieId = parseInt(params.id, 10);
  const [releases, setReleases] = useState<Release[]>([]);
  const [isSearching, setIsSearching] = useState(false);

  // Fetch movie details
  const { data: movie, isLoading, isError, error } = useQuery({
    queryKey: ['movies', movieId],
    queryFn: () => api.getMovie(movieId),
  });

  // Search releases mutation
  const searchMutation = useMutation({
    mutationFn: () => api.searchMovieReleases(movieId),
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
      api.downloadMovie(movieId, { release_id: releaseId }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['movies', movieId] });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    mutationFn: () => api.deleteMovie(movieId, false),
    onSuccess: () => {
      router.push('/movies');
    },
  });

  const handleSearchReleases = () => {
    setIsSearching(true);
    searchMutation.mutate();
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-12 w-12 animate-spin text-lcars-orange" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className="space-y-4">
        <LcarsButton variant="orange" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-status-missing">
            Failed to load movie: {error?.message || 'Unknown error'}
          </p>
        </div>
      </div>
    );
  }

  if (!movie) {
    return (
      <div className="space-y-4">
        <LcarsButton variant="orange" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-lg text-lcars-text-dim">Movie not found</p>
        </div>
      </div>
    );
  }

  const posterUrl = movie.poster_path
    ? `https://image.tmdb.org/t/p/w500${movie.poster_path}`
    : '/placeholder-poster.png';

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <LcarsButton variant="orange" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <LcarsButton variant="red" onClick={() => deleteMutation.mutate()}>
          <Trash2 className="mr-2 h-5 w-5" />
          Delete
        </LcarsButton>
      </div>

      {/* Movie Info */}
      <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
        {/* Poster */}
        <div className="md:col-span-1">
          <div className="relative aspect-[2/3] overflow-hidden rounded-lcars">
            <Image
              src={posterUrl}
              alt={movie.title}
              fill
              className="object-cover"
              priority
              sizes="(max-width: 768px) 100vw, 33vw"
            />
          </div>
        </div>

        {/* Details */}
        <div className="space-y-4 md:col-span-2">
          <div>
            <h1 className="text-3xl font-bold uppercase tracking-wider text-lcars-orange">
              {movie.title}
            </h1>
            <p className="mt-1 text-lg text-lcars-text-dim">{movie.year}</p>
          </div>

          {/* Status Panel */}
          <LcarsPanel accentColor="orange">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Status:</span>
                <div className="flex items-center gap-2">
                  <div
                    className={cn(
                      'h-3 w-3 rounded-full',
                      mediaStatusColors[movie.status]
                    )}
                  />
                  <span className="text-sm font-bold uppercase text-lcars-text">
                    {movie.status}
                  </span>
                </div>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Monitored:</span>
                <span className="text-sm font-bold text-lcars-text">
                  {movie.monitored ? 'YES' : 'NO'}
                </span>
              </div>
              {movie.file_path && (
                <div className="flex items-center justify-between">
                  <span className="text-sm text-lcars-text-dim">File Size:</span>
                  <span className="text-sm font-bold text-lcars-text">
                    {formatBytes(movie.file_size || 0)}
                  </span>
                </div>
              )}
            </div>
          </LcarsPanel>

          {/* Overview */}
          {movie.overview && (
            <LcarsPanel title="Overview" accentColor="blue">
              <p className="text-sm leading-relaxed text-lcars-text">{movie.overview}</p>
            </LcarsPanel>
          )}

          {/* Metadata */}
          <LcarsPanel title="Details" accentColor="purple">
            <div className="space-y-2 text-sm">
              {movie.runtime_minutes && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Runtime:</span>
                  <span className="text-lcars-text">{movie.runtime_minutes} min</span>
                </div>
              )}
              {movie.genres.length > 0 && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">Genres:</span>
                  <span className="text-lcars-text">{movie.genres.join(', ')}</span>
                </div>
              )}
              {movie.imdb_id && (
                <div className="flex justify-between">
                  <span className="text-lcars-text-dim">IMDB ID:</span>
                  <span className="text-lcars-text">{movie.imdb_id}</span>
                </div>
              )}
            </div>
          </LcarsPanel>
        </div>
      </div>

      {/* Search Releases */}
      <div>
        <LcarsButton
          variant="orange"
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
          <h2 className="mb-4 text-xl font-bold uppercase tracking-wider text-lcars-orange">
            Available Releases ({releases.length})
          </h2>
          <div className="space-y-3">
            {releases.map((release) => (
              <div
                key={release.id}
                className="rounded-lcars bg-lcars-dark p-4"
              >
                <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex-1">
                    <h3 className="text-sm font-bold text-lcars-text">
                      {release.title}
                    </h3>
                    <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-lcars-text-dim">
                      <span>Size: {formatBytes(release.size_bytes)}</span>
                      <span>Quality: {release.quality}</span>
                      <span>Source: {release.source}</span>
                      <span>Seeders: {release.seeders}</span>
                      {release.group && <span>Group: {release.group}</span>}
                    </div>
                  </div>
                  <LcarsButton
                    variant="orange"
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
