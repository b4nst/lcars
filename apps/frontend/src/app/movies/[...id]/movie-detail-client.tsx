'use client';

import { useParams } from 'next/navigation';
import { MovieDetail } from './movie-detail';

export function MovieDetailClient() {
  const params = useParams();
  const idSegments = params.id as string[];
  const id = idSegments[0];

  return <MovieDetail id={id} />;
}
