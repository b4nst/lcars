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
import type {
  TmdbMovieSearchResult,
  TmdbTvSearchResult,
  MusicBrainzArtistSearchResult,
  MusicBrainzAlbumSearchResult,
  ShowStatus,
} from '@/lib/types';

const STATUS_FILTERS: Array<{ label: string; value: ShowStatus | 'all' }> = [
  { label: 'All', value: 'all' },
  { label: 'Continuing', value: 'continuing' },
  { label: 'Ended', value: 'ended' },
  { label: 'Canceled', value: 'canceled' },
  { label: 'Upcoming', value: 'upcoming' },
];

export default function TvShowsPage() {
  const [statusFilter, setStatusFilter] = useState<ShowStatus | 'all'>('all');
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const queryClient = useQueryClient();

  // Fetch TV shows with filters
  const { data: showsResponse, isLoading, isError, error } = useQuery({
    queryKey: ['tv', statusFilter],
    queryFn: () =>
      api.getTvShows({
        status: statusFilter !== 'all' ? statusFilter : undefined,
      }),
  });

  // Add show mutation
  const addShowMutation = useMutation({
    mutationFn: (tmdbId: number) =>
      api.addTvShow({
        tmdb_id: tmdbId,
        monitored: true,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tv'] });
      setIsSearchOpen(false);
    },
  });

  const handleAddShow = (result: TmdbMovieSearchResult | TmdbTvSearchResult | MusicBrainzArtistSearchResult | MusicBrainzAlbumSearchResult) => {
    // Type narrowing: this handler is only used with TV searches
    if ('name' in result && typeof result.id === 'number') {
      addShowMutation.mutate(result.id);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-orange">
          TV Shows
        </h1>
        <LcarsButton variant="orange" onClick={() => setIsSearchOpen(true)}>
          <Plus className="mr-2 h-5 w-5" />
          Add Show
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

      {/* Shows Grid */}
      {isError ? (
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-status-missing">
            Failed to load TV shows: {error?.message || 'Unknown error'}
          </p>
        </div>
      ) : isLoading ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-12 w-12 animate-spin text-lcars-orange" />
        </div>
      ) : showsResponse && showsResponse.items.length > 0 ? (
        <div>
          <div className="mb-4 text-sm text-lcars-text-dim">
            {showsResponse.total} {showsResponse.total === 1 ? 'show' : 'shows'}
          </div>
          <MediaGrid>
            {showsResponse.items.map((show) => (
              <Link key={show.id} href={`/tv/${show.id}`}>
                <MediaCard media={show} />
              </Link>
            ))}
          </MediaGrid>
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center rounded-lcars bg-lcars-dark py-12">
          <p className="text-lg text-lcars-text-dim">No TV shows found</p>
          <LcarsButton
            variant="orange"
            className="mt-4"
            onClick={() => setIsSearchOpen(true)}
          >
            <Plus className="mr-2 h-5 w-5" />
            Add Your First Show
          </LcarsButton>
        </div>
      )}

      {/* Search Modal */}
      <SearchModal
        isOpen={isSearchOpen}
        onClose={() => setIsSearchOpen(false)}
        searchType="tv"
        onAdd={handleAddShow}
      />
    </div>
  );
}
