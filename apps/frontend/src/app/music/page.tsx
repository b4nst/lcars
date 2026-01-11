'use client';

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import Link from 'next/link';
import { Plus, Loader2 } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import { MediaGrid } from '@/components/media-grid';
import { ArtistCard } from '@/components/artist-card';
import { SearchModal } from '@/components/search-modal';
import type {
  TmdbMovieSearchResult,
  TmdbTvSearchResult,
  MusicBrainzArtistSearchResult,
  MusicBrainzAlbumSearchResult,
} from '@/lib/types';

export default function MusicPage() {
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const queryClient = useQueryClient();

  // Fetch artists
  const { data: artistsResponse, isLoading, isError, error } = useQuery({
    queryKey: ['artists'],
    queryFn: () => api.getArtists(),
  });

  // Add artist mutation
  const addArtistMutation = useMutation({
    mutationFn: (mbid: string) =>
      api.addArtist({
        mbid,
        monitored: true,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['artists'] });
      setIsSearchOpen(false);
    },
  });

  const handleAddArtist = (result: TmdbMovieSearchResult | TmdbTvSearchResult | MusicBrainzArtistSearchResult | MusicBrainzAlbumSearchResult) => {
    // Type narrowing: this handler is only used with artist searches
    // MusicBrainz results use string IDs while TMDB uses numbers
    if ('name' in result && typeof result.id === 'string') {
      addArtistMutation.mutate(result.id);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-2xl font-bold uppercase tracking-wider text-lcars-purple">
          Music
        </h1>
        <LcarsButton variant="purple" onClick={() => setIsSearchOpen(true)}>
          <Plus className="mr-2 h-5 w-5" />
          Add Artist
        </LcarsButton>
      </div>

      {/* Artists Grid */}
      {isError ? (
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-status-missing">
            Failed to load artists: {error?.message || 'Unknown error'}
          </p>
        </div>
      ) : isLoading ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-12 w-12 animate-spin text-lcars-purple" />
        </div>
      ) : artistsResponse && artistsResponse.items.length > 0 ? (
        <div>
          <div className="mb-4 text-sm text-lcars-text-dim">
            {artistsResponse.total} {artistsResponse.total === 1 ? 'artist' : 'artists'}
          </div>
          <MediaGrid>
            {artistsResponse.items.map((artist) => (
              <Link key={artist.id} href={`/music/artists/${artist.id}`}>
                <ArtistCard artist={artist} />
              </Link>
            ))}
          </MediaGrid>
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center rounded-lcars bg-lcars-dark py-12">
          <p className="text-lg text-lcars-text-dim">No artists found</p>
          <LcarsButton
            variant="purple"
            className="mt-4"
            onClick={() => setIsSearchOpen(true)}
          >
            <Plus className="mr-2 h-5 w-5" />
            Add Your First Artist
          </LcarsButton>
        </div>
      )}

      {/* Search Modal */}
      <SearchModal
        isOpen={isSearchOpen}
        onClose={() => setIsSearchOpen(false)}
        searchType="artist"
        onAdd={handleAddArtist}
      />
    </div>
  );
}
