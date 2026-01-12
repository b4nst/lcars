import { AlbumDetail } from './album-detail';

// Required for static export - pages will be client-rendered
export function generateStaticParams() {
  return [];
}

interface AlbumDetailPageProps {
  params: {
    id: string;
  };
}

export default function AlbumDetailPage({ params }: AlbumDetailPageProps) {
  return <AlbumDetail id={params.id} />;
}
