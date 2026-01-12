'use client';

import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import Image from 'next/image';
import { ArrowLeft, Search, Download, Trash2, Loader2, ChevronDown, ChevronRight } from 'lucide-react';
import { api } from '@/lib/api';
import { LcarsButton } from '@/components/lcars/button';
import { LcarsPanel } from '@/components/lcars/panel';
import { cn, formatBytes } from '@/lib/utils';
import { mediaStatusColors } from '@/lib/constants';
import type { Release, Episode } from '@/lib/types';

interface TvShowDetailProps {
  id: string;
}

export function TvShowDetail({ id }: TvShowDetailProps) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const showId = parseInt(id, 10);
  const [expandedSeasons, setExpandedSeasons] = useState<Set<number>>(new Set([1]));
  const [selectedEpisode, setSelectedEpisode] = useState<Episode | null>(null);
  const [releases, setReleases] = useState<Release[]>([]);
  const [isSearching, setIsSearching] = useState(false);

  // Fetch show details
  const { data: show, isLoading, isError, error } = useQuery({
    queryKey: ['tv', showId],
    queryFn: () => api.getTvShow(showId),
  });

  // Search episode releases mutation
  const searchMutation = useMutation({
    mutationFn: (episode: Episode) =>
      api.searchEpisodeReleases(showId, episode.season_number, episode.episode_number),
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
    mutationFn: ({ episode, releaseId }: { episode: Episode; releaseId: string }) =>
      api.downloadEpisode(showId, episode.season_number, episode.episode_number, {
        release_id: releaseId,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tv', showId] });
    },
  });

  // Delete mutation
  const deleteMutation = useMutation({
    mutationFn: () => api.deleteTvShow(showId, false),
    onSuccess: () => {
      router.push('/tv');
    },
  });

  const toggleSeason = (seasonNumber: number) => {
    setExpandedSeasons((prev) => {
      const next = new Set(prev);
      if (next.has(seasonNumber)) {
        next.delete(seasonNumber);
      } else {
        next.add(seasonNumber);
      }
      return next;
    });
  };

  const handleSearchEpisode = (episode: Episode) => {
    setSelectedEpisode(episode);
    setIsSearching(true);
    searchMutation.mutate(episode);
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
            Failed to load show: {error?.message || 'Unknown error'}
          </p>
        </div>
      </div>
    );
  }

  if (!show) {
    return (
      <div className="space-y-4">
        <LcarsButton variant="orange" onClick={() => router.back()}>
          <ArrowLeft className="mr-2 h-5 w-5" />
          Back
        </LcarsButton>
        <div className="rounded-lcars bg-lcars-dark p-8 text-center">
          <p className="text-lg text-lcars-text-dim">Show not found</p>
        </div>
      </div>
    );
  }

  const posterUrl = show.poster_path
    ? `https://image.tmdb.org/t/p/w500${show.poster_path}`
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

      {/* Show Info */}
      <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
        {/* Poster */}
        <div className="md:col-span-1">
          <div className="relative aspect-[2/3] overflow-hidden rounded-lcars">
            <Image
              src={posterUrl}
              alt={show.title}
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
              {show.title}
            </h1>
            <p className="mt-1 text-lg text-lcars-text-dim">
              {show.year_start}
              {show.year_end ? ` - ${show.year_end}` : ' - Present'}
            </p>
          </div>

          {/* Status Panel */}
          <LcarsPanel accentColor="orange">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Status:</span>
                <span className="text-sm font-bold uppercase text-lcars-text">
                  {show.status}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Monitored:</span>
                <span className="text-sm font-bold text-lcars-text">
                  {show.monitored ? 'YES' : 'NO'}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-sm text-lcars-text-dim">Seasons:</span>
                <span className="text-sm font-bold text-lcars-text">
                  {show.seasons.length}
                </span>
              </div>
            </div>
          </LcarsPanel>

          {/* Overview */}
          {show.overview && (
            <LcarsPanel title="Overview" accentColor="blue">
              <p className="text-sm leading-relaxed text-lcars-text">{show.overview}</p>
            </LcarsPanel>
          )}
        </div>
      </div>

      {/* Seasons & Episodes */}
      <div>
        <h2 className="mb-4 text-xl font-bold uppercase tracking-wider text-lcars-orange">
          Seasons & Episodes
        </h2>
        <div className="space-y-3">
          {show.seasons.map((season) => (
            <div key={season.season_number} className="rounded-lcars bg-lcars-dark">
              {/* Season Header */}
              <button
                onClick={() => toggleSeason(season.season_number)}
                className="flex w-full items-center justify-between p-4 text-left transition-colors hover:bg-lcars-orange/10"
              >
                <div className="flex items-center gap-3">
                  {expandedSeasons.has(season.season_number) ? (
                    <ChevronDown className="h-5 w-5 text-lcars-orange" />
                  ) : (
                    <ChevronRight className="h-5 w-5 text-lcars-orange" />
                  )}
                  <h3 className="text-lg font-bold text-lcars-text">
                    Season {season.season_number}
                  </h3>
                </div>
                <div className="text-sm text-lcars-text-dim">
                  {season.available_count} / {season.episode_count} episodes
                </div>
              </button>

              {/* Episodes List */}
              {expandedSeasons.has(season.season_number) && (
                <div className="border-t border-lcars-orange/30 p-4">
                  <div className="space-y-2">
                    {season.episodes.map((episode) => (
                      <div
                        key={episode.id}
                        className="flex items-center justify-between rounded-lg bg-lcars-black p-3"
                      >
                        <div className="flex flex-1 items-center gap-3">
                          <div
                            className={cn(
                              'h-3 w-3 shrink-0 rounded-full',
                              mediaStatusColors[episode.status]
                            )}
                            title={episode.status}
                          />
                          <div className="flex-1">
                            <div className="flex items-baseline gap-2">
                              <span className="text-sm font-bold text-lcars-text">
                                {episode.episode_number}.
                              </span>
                              <span className="text-sm text-lcars-text">
                                {episode.title || `Episode ${episode.episode_number}`}
                              </span>
                            </div>
                            {episode.air_date && (
                              <p className="mt-1 text-xs text-lcars-text-dim">
                                {new Date(episode.air_date).toLocaleDateString()}
                              </p>
                            )}
                          </div>
                        </div>
                        <LcarsButton
                          variant="orange"
                          size="sm"
                          onClick={() => handleSearchEpisode(episode)}
                        >
                          <Search className="h-4 w-4" />
                        </LcarsButton>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Releases List */}
      {releases.length > 0 && selectedEpisode && (
        <div>
          <h2 className="mb-4 text-xl font-bold uppercase tracking-wider text-lcars-orange">
            Releases for S{selectedEpisode.season_number}E{selectedEpisode.episode_number} (
            {releases.length})
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
                    variant="orange"
                    size="sm"
                    onClick={() =>
                      downloadMutation.mutate({
                        episode: selectedEpisode,
                        releaseId: release.id,
                      })
                    }
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
