import { ArtistDetail } from './artist-detail';

// Required for static export - pages will be client-rendered
export function generateStaticParams() {
  return [];
}

interface ArtistDetailPageProps {
  params: {
    id: string;
  };
}

export default function ArtistDetailPage({ params }: ArtistDetailPageProps) {
  return <ArtistDetail id={params.id} />;
}
