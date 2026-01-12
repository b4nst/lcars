'use client';

import { useParams } from 'next/navigation';
import { AlbumDetail } from './album-detail';

export function AlbumDetailClient() {
  const params = useParams();
  const idSegments = params.id as string[];
  const id = idSegments[0];

  return <AlbumDetail id={id} />;
}
