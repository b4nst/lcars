'use client';

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import Link from 'next/link';
import { Plus, Loader2 } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import { MediaGrid } from '@/components/media-grid';
import { MediaCard } from '@/components/media-card';
import { SearchModal } from '@/components/search-modal';
import { cn } from '@/lib/utils';
import type {
  TmdbMovieSearchResult,
  TmdbTvSearchResult,
  MusicBrainzArtistSearchResult,
  MusicBrainzAlbumSearchResult,
  MediaStatus,
} from '@/lib/types';

const STATUS_FILTERS: Array<{ label: string; value: MediaStatus | 'all' }> = [
  { label: 'All', value: 'all' },
  { label: 'Available', value: 'available' },
  { label: 'Missing', value: 'missing' },
  { label: 'Downloading', value: 'downloading' },
];

export default function MoviesPage() {
  const [statusFilter, setStatusFilter] = useState<MediaStatus | 'all'>('all');
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const queryClient = useQueryClient();

  // Fetch movies with filters
  const { data: moviesResponse, isLoading, isError, error } = useQuery({
    queryKey: ['movies', statusFilter],
    queryFn: () =>
      api.getMovies({
        status: statusFilter !== 'all' ? statusFilter : undefined,
      }),
  });

  // Add movie mutation
  const addMovieMutation = useMutation({
    mutationFn: (tmdbId: number) =>
      api.addMovie({
        tmdb_id: tmdbId,
        monitored: true,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['movies'] });
      setIsSearchOpen(false);
    },
  });

  const handleAddMovie = (result: TmdbMovieSearchResult | TmdbTvSearchResult | MusicBrainzArtistSearchResult | MusicBrainzAlbumSearchResult) => {
    // Type narrowing: this handler is only used with movie searches
    if ('title' in result && typeof result.id === 'number') {
      addMovieMutation.mutate(result.id);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          Movies
        </h1>
        <LcarsButton variant="orange" onClick={() => setIsSearchOpen(true)}>
          <Plus className="mr-2 h-5 w-5" />
          Add Movie
        </LcarsButton>
      </div>

      {/* Status Filters */}
      <div className="flex flex-wrap gap-2">
        {STATUS_FILTERS.map((filter) => (
          <LcarsButton
            key={filter.value}
            variant={statusFilter === filter.value ? 'orange' : 'yellow'}
            size="sm"
            onClick={() => setStatusFilter(filter.value)}
          >
            {filter.label}
          </LcarsButton>
        ))}
      </div>

      {/* Movies Grid */}
      {isError ? (
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-status-missing">
            Failed to load movies: {error?.message || 'Unknown error'}
          </p>
        </div>
      ) : isLoading ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-12 w-12 animate-spin text-lcars-orange" />
        </div>
      ) : moviesResponse && moviesResponse.items.length > 0 ? (
        <div>
          <div className="mb-4 text-sm text-lcars-text-dim">
            {moviesResponse.total} {moviesResponse.total === 1 ? 'movie' : 'movies'}
          </div>
          <MediaGrid>
            {moviesResponse.items.map((movie) => (
              <Link key={movie.id} href={`/movies/${movie.id}`}>
                <MediaCard media={movie} />
              </Link>
            ))}
          </MediaGrid>
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center rounded-lcars bg-lcars-dark py-12">
          <p className="text-lg text-lcars-text-dim">No movies found</p>
          <LcarsButton
            variant="orange"
            className="mt-4"
            onClick={() => setIsSearchOpen(true)}
          >
            <Plus className="mr-2 h-5 w-5" />
            Add Your First Movie
          </LcarsButton>
        </div>
      )}

      {/* Search Modal */}
      <SearchModal
        isOpen={isSearchOpen}
        onClose={() => setIsSearchOpen(false)}
        searchType="movie"
        onAdd={handleAddMovie}
      />
    </div>
  );
}
