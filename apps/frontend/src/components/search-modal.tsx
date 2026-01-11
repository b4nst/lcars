'use client';

import { useState, useEffect, useCallback, forwardRef, HTMLAttributes } from 'react';
import { useQuery } from '@tanstack/react-query';
import Image from 'next/image';
import { Search, X, Plus, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import type {
  TmdbMovieSearchResult,
  TmdbTvSearchResult,
  MusicBrainzArtistSearchResult,
  MusicBrainzAlbumSearchResult,
} from '@/lib/types';

type SearchType = 'movie' | 'tv' | 'artist' | 'album';
type SearchResult =
  | TmdbMovieSearchResult
  | TmdbTvSearchResult
  | MusicBrainzArtistSearchResult
  | MusicBrainzAlbumSearchResult;

interface SearchModalProps extends HTMLAttributes<HTMLDivElement> {
  isOpen: boolean;
  onClose: () => void;
  searchType: SearchType;
  onAdd: (result: SearchResult) => void;
}

export const SearchModal = forwardRef<HTMLDivElement, SearchModalProps>(
  ({ isOpen, onClose, searchType, onAdd, className, ...props }, ref) => {
    const [query, setQuery] = useState('');
    const [debouncedQuery, setDebouncedQuery] = useState('');

    // Debounce search query (300ms)
    useEffect(() => {
      const handler = setTimeout(() => {
        setDebouncedQuery(query);
      }, 300);

      return () => {
        clearTimeout(handler);
      };
    }, [query]);

    // Handle Escape key to close modal
    useEffect(() => {
      const handleEscape = (e: KeyboardEvent) => {
        if (e.key === 'Escape' && isOpen) {
          onClose();
        }
      };

      document.addEventListener('keydown', handleEscape);
      return () => {
        document.removeEventListener('keydown', handleEscape);
      };
    }, [isOpen, onClose]);

    // Search query
    const { data: results = [], isLoading } = useQuery({
      queryKey: ['search', searchType, debouncedQuery],
      queryFn: async () => {
        if (!debouncedQuery.trim()) return [];

        switch (searchType) {
          case 'movie':
            return api.searchTmdbMovies(debouncedQuery);
          case 'tv':
            return api.searchTmdbTv(debouncedQuery);
          case 'artist':
            return api.searchMusicBrainzArtists(debouncedQuery);
          case 'album':
            return api.searchMusicBrainzAlbums(debouncedQuery);
          default:
            return [];
        }
      },
      enabled: isOpen && debouncedQuery.length > 0,
    });

    const handleAdd = useCallback((result: SearchResult) => {
      onAdd(result);
      setQuery('');
      setDebouncedQuery('');
    }, [onAdd]);

    if (!isOpen) return null;

    return (
      <>
        {/* Backdrop */}
        <div
          className="fixed inset-0 z-40 bg-lcars-black/80 backdrop-blur-sm"
          onClick={onClose}
        />

        {/* Modal */}
        <div
          ref={ref}
          role="dialog"
          aria-modal="true"
          aria-labelledby="search-modal-title"
          className={cn(
            'fixed left-1/2 top-1/2 z-50 w-full max-w-2xl -translate-x-1/2 -translate-y-1/2',
            'max-h-[80vh] overflow-hidden rounded-lcars bg-lcars-dark shadow-xl',
            className
          )}
          {...props}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-lcars-orange p-4">
            <h2 id="search-modal-title" className="text-lg font-bold text-lcars-orange">
              SEARCH {searchType.toUpperCase()}
            </h2>
            <LcarsButton variant="red" size="sm" onClick={onClose}>
              <X className="h-5 w-5" />
            </LcarsButton>
          </div>

          {/* Search Input */}
          <div className="border-b border-lcars-orange/30 p-4">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 h-5 w-5 -translate-y-1/2 text-lcars-text-dim" />
              <input
                type="text"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={`Search for ${searchType}...`}
                className={cn(
                  'w-full rounded-lcars bg-lcars-black py-3 pl-10 pr-4',
                  'font-lcars text-lcars-text uppercase tracking-wider',
                  'placeholder:text-lcars-text-dim placeholder:normal-case',
                  'focus:outline-none focus:ring-2 focus:ring-lcars-orange'
                )}
                autoFocus
              />
              {isLoading && (
                <Loader2 className="absolute right-3 top-1/2 h-5 w-5 -translate-y-1/2 animate-spin text-lcars-orange" />
              )}
            </div>
          </div>

          {/* Results */}
          <div className="max-h-96 overflow-y-auto p-4" aria-live="polite">
            {!debouncedQuery ? (
              <div className="py-8 text-center text-sm text-lcars-text-dim">
                Enter a search query to begin
              </div>
            ) : results.length === 0 && !isLoading ? (
              <div className="py-8 text-center text-sm text-lcars-text-dim">
                No results found
              </div>
            ) : (
              <div className="space-y-2">
                {results.map((result) => (
                  <SearchResultItem
                    key={'id' in result ? result.id : (result as MusicBrainzArtistSearchResult | MusicBrainzAlbumSearchResult).id}
                    result={result}
                    searchType={searchType}
                    onAdd={() => handleAdd(result)}
                  />
                ))}
              </div>
            )}
          </div>
        </div>
      </>
    );
  }
);

SearchModal.displayName = 'SearchModal';

// Result Item Component
interface SearchResultItemProps {
  result: SearchResult;
  searchType: SearchType;
  onAdd: () => void;
}

function SearchResultItem({ result, searchType, onAdd }: SearchResultItemProps) {
  const getImageUrl = () => {
    if (searchType === 'movie' || searchType === 'tv') {
      const tmdbResult = result as TmdbMovieSearchResult | TmdbTvSearchResult;
      return tmdbResult.poster_path
        ? `https://image.tmdb.org/t/p/w200${tmdbResult.poster_path}`
        : null;
    }
    return null;
  };

  const getTitle = () => {
    if ('title' in result) return result.title;
    if ('name' in result) return result.name;
    return 'Unknown';
  };

  const getSubtitle = () => {
    if (searchType === 'movie') {
      const movie = result as TmdbMovieSearchResult;
      return movie.release_date ? new Date(movie.release_date).getFullYear() : null;
    }
    if (searchType === 'tv') {
      const tv = result as TmdbTvSearchResult;
      return tv.first_air_date ? new Date(tv.first_air_date).getFullYear() : null;
    }
    if (searchType === 'artist') {
      const artist = result as MusicBrainzArtistSearchResult;
      return artist.disambiguation || artist.country || null;
    }
    if (searchType === 'album') {
      const album = result as MusicBrainzAlbumSearchResult;
      return album.artist_credit || null;
    }
    return null;
  };

  const imageUrl = getImageUrl();
  const title = getTitle();
  const subtitle = getSubtitle();

  return (
    <div className="flex items-center gap-3 rounded-lcars bg-lcars-black p-3 transition-colors hover:bg-lcars-orange/10">
      {/* Poster/Image */}
      {imageUrl ? (
        <div className="relative h-16 w-12 shrink-0 overflow-hidden rounded">
          <Image
            src={imageUrl}
            alt={title}
            fill
            className="object-cover"
            sizes="48px"
          />
        </div>
      ) : (
        <div className="flex h-16 w-12 shrink-0 items-center justify-center rounded bg-lcars-dark">
          <Search className="h-6 w-6 text-lcars-text-dim" />
        </div>
      )}

      {/* Info */}
      <div className="flex-1">
        <h3 className="text-sm font-bold text-lcars-text">{title}</h3>
        {subtitle && (
          <p className="mt-1 text-xs text-lcars-text-dim">{subtitle}</p>
        )}
      </div>

      {/* Add Button */}
      <LcarsButton variant="orange" size="sm" onClick={onAdd}>
        <Plus className="h-4 w-4" />
        <span className="ml-1">Add</span>
      </LcarsButton>
    </div>
  );
}
