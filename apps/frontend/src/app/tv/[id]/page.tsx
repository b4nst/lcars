import { TvShowDetail } from './tv-detail';

// Required for static export - pages will be client-rendered
export function generateStaticParams() {
  return [];
}

interface TvShowDetailPageProps {
  params: {
    id: string;
  };
}

export default function TvShowDetailPage({ params }: TvShowDetailPageProps) {
  return <TvShowDetail id={params.id} />;
}
