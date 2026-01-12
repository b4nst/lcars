'use client';

import { useParams } from 'next/navigation';
import { TvShowDetail } from './tv-detail';

export function TvShowDetailClient() {
  const params = useParams();
  const idSegments = params.id as string[];
  const id = idSegments[0];

  return <TvShowDetail id={id} />;
}
