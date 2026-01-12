import { MovieDetail } from './movie-detail';

// Required for static export - pages will be client-rendered
export function generateStaticParams() {
  return [];
}

interface MovieDetailPageProps {
  params: {
    id: string;
  };
}

export default function MovieDetailPage({ params }: MovieDetailPageProps) {
  return <MovieDetail id={params.id} />;
}
