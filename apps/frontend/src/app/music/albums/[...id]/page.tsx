import { AlbumDetailClient } from './album-detail-client';

// Generate a placeholder static param - the actual ID will be handled client-side
export function generateStaticParams() {
  return [{ id: ['_'] }];
}

export default function AlbumDetailPage() {
  return <AlbumDetailClient />;
}
