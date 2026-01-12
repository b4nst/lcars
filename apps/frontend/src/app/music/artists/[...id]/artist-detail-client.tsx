'use client';

import { useParams } from 'next/navigation';
import { ArtistDetail } from './artist-detail';

export function ArtistDetailClient() {
  const params = useParams();
  const idSegments = params.id as string[];
  const id = idSegments[0];

  return <ArtistDetail id={id} />;
}
