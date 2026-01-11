import type {
  User,
  Movie,
  TvShow,
  Episode,
  Artist,
  Album,
  Track,
  Download,
  Release,
  SystemStatus,
  PaginatedResponse,
  TmdbMovieSearchResult,
  TmdbTvSearchResult,
  MusicBrainzArtistSearchResult,
  MusicBrainzAlbumSearchResult,
  ActivityEvent,
} from './types';

// Custom error class for API errors
export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly code?: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export class AuthError extends ApiError {
  constructor(message: string = 'Authentication required') {
    super(message, 401, 'AUTH_ERROR');
    this.name = 'AuthError';
  }
}

export class ApiClient {
  private baseUrl: string;
  private token: string | null = null;

  constructor(baseUrl: string = '/api') {
    this.baseUrl = baseUrl;
  }

  setToken(token: string | null) {
    this.token = token;
  }

  getToken(): string | null {
    return this.token;
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit & { signal?: AbortSignal } = {}
  ): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    const response = await fetch(url, {
      ...options,
      signal: options.signal,
      headers: {
        ...headers,
        ...(options.headers as Record<string, string>),
      },
    });

    if (!response.ok) {
      // Handle authentication errors specially
      if (response.status === 401) {
        throw new AuthError();
      }

      // Don't expose full error details in production
      const errorText = await response.text();
      const message = process.env.NODE_ENV === 'production'
        ? `Request failed with status ${response.status}`
        : errorText || `HTTP ${response.status}`;

      throw new ApiError(message, response.status);
    }

    // Handle empty responses
    const text = await response.text();
    return text ? JSON.parse(text) : (null as T);
  }

  // Auth endpoints
  async login(username: string, password: string): Promise<{ token: string; user: User }> {
    return this.request('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username, password }),
    });
  }

  async logout(): Promise<{ success: boolean }> {
    return this.request('/auth/logout', { method: 'POST' });
  }

  async me(): Promise<User> {
    return this.request('/auth/me');
  }

  // Movies endpoints
  async getMovies(params?: {
    status?: string;
    monitored?: boolean;
    search?: string;
    page?: number;
    limit?: number;
  }): Promise<PaginatedResponse<Movie>> {
    const searchParams = new URLSearchParams();
    if (params?.status) searchParams.append('status', params.status);
    if (params?.monitored !== undefined) searchParams.append('monitored', String(params.monitored));
    if (params?.search) searchParams.append('search', params.search);
    if (params?.page) searchParams.append('page', String(params.page));
    if (params?.limit) searchParams.append('limit', String(params.limit));

    const query = searchParams.toString();
    return this.request(`/movies${query ? `?${query}` : ''}`);
  }

  async getMovie(id: number): Promise<Movie> {
    return this.request(`/movies/${id}`);
  }

  async addMovie(data: {
    tmdb_id: number;
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<Movie> {
    return this.request('/movies', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async updateMovie(id: number, data: {
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<Movie> {
    return this.request(`/movies/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  async deleteMovie(id: number, deleteFiles: boolean = false): Promise<{ success: boolean }> {
    return this.request(`/movies/${id}?delete_files=${deleteFiles}`, {
      method: 'DELETE',
    });
  }

  async searchMovieReleases(id: number): Promise<Release[]> {
    return this.request(`/movies/${id}/search`, { method: 'POST' });
  }

  async downloadMovie(id: number, data: { release_id?: string; magnet?: string }): Promise<Download> {
    return this.request(`/movies/${id}/download`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  // TV Shows endpoints
  async getTvShows(params?: {
    status?: string;
    monitored?: boolean;
    search?: string;
    page?: number;
    limit?: number;
  }): Promise<PaginatedResponse<TvShow>> {
    const searchParams = new URLSearchParams();
    if (params?.status) searchParams.append('status', params.status);
    if (params?.monitored !== undefined) searchParams.append('monitored', String(params.monitored));
    if (params?.search) searchParams.append('search', params.search);
    if (params?.page) searchParams.append('page', String(params.page));
    if (params?.limit) searchParams.append('limit', String(params.limit));

    const query = searchParams.toString();
    return this.request(`/tv${query ? `?${query}` : ''}`);
  }

  async getTvShow(id: number): Promise<TvShow> {
    return this.request(`/tv/${id}`);
  }

  async addTvShow(data: {
    tmdb_id: number;
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<TvShow> {
    return this.request('/tv', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async updateTvShow(id: number, data: {
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<TvShow> {
    return this.request(`/tv/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  async deleteTvShow(id: number, deleteFiles: boolean = false): Promise<{ success: boolean }> {
    return this.request(`/tv/${id}?delete_files=${deleteFiles}`, {
      method: 'DELETE',
    });
  }

  async updateEpisode(
    showId: number,
    seasonNumber: number,
    episodeNumber: number,
    data: { monitored?: boolean }
  ): Promise<Episode> {
    return this.request(`/tv/${showId}/season/${seasonNumber}/episode/${episodeNumber}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  async searchEpisodeReleases(
    showId: number,
    seasonNumber: number,
    episodeNumber: number
  ): Promise<Release[]> {
    return this.request(
      `/tv/${showId}/season/${seasonNumber}/episode/${episodeNumber}/search`,
      { method: 'POST' }
    );
  }

  async downloadEpisode(
    showId: number,
    seasonNumber: number,
    episodeNumber: number,
    data: { release_id?: string; magnet?: string }
  ): Promise<Download> {
    return this.request(
      `/tv/${showId}/season/${seasonNumber}/episode/${episodeNumber}/download`,
      {
        method: 'POST',
        body: JSON.stringify(data),
      }
    );
  }

  // Music - Artists endpoints
  async getArtists(params?: {
    monitored?: boolean;
    search?: string;
    page?: number;
    limit?: number;
  }): Promise<PaginatedResponse<Artist>> {
    const searchParams = new URLSearchParams();
    if (params?.monitored !== undefined) searchParams.append('monitored', String(params.monitored));
    if (params?.search) searchParams.append('search', params.search);
    if (params?.page) searchParams.append('page', String(params.page));
    if (params?.limit) searchParams.append('limit', String(params.limit));

    const query = searchParams.toString();
    return this.request(`/artists${query ? `?${query}` : ''}`);
  }

  async getArtist(id: number): Promise<Artist> {
    return this.request(`/artists/${id}`);
  }

  async addArtist(data: {
    mbid: string;
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<Artist> {
    return this.request('/artists', {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async updateArtist(id: number, data: {
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<Artist> {
    return this.request(`/artists/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  async deleteArtist(id: number, deleteFiles: boolean = false): Promise<{ success: boolean }> {
    return this.request(`/artists/${id}?delete_files=${deleteFiles}`, {
      method: 'DELETE',
    });
  }

  // Music - Albums endpoints
  async getAlbums(params?: {
    artist_id?: number;
    status?: string;
    monitored?: boolean;
    search?: string;
    page?: number;
    limit?: number;
  }): Promise<PaginatedResponse<Album>> {
    const searchParams = new URLSearchParams();
    if (params?.artist_id) searchParams.append('artist_id', String(params.artist_id));
    if (params?.status) searchParams.append('status', params.status);
    if (params?.monitored !== undefined) searchParams.append('monitored', String(params.monitored));
    if (params?.search) searchParams.append('search', params.search);
    if (params?.page) searchParams.append('page', String(params.page));
    if (params?.limit) searchParams.append('limit', String(params.limit));

    const query = searchParams.toString();
    return this.request(`/albums${query ? `?${query}` : ''}`);
  }

  async getAlbum(id: number): Promise<Album> {
    return this.request(`/albums/${id}`);
  }

  async updateAlbum(id: number, data: {
    monitored?: boolean;
    quality_limit?: string;
  }): Promise<Album> {
    return this.request(`/albums/${id}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  async deleteAlbum(id: number, deleteFiles: boolean = false): Promise<{ success: boolean }> {
    return this.request(`/albums/${id}?delete_files=${deleteFiles}`, {
      method: 'DELETE',
    });
  }

  async searchAlbumReleases(id: number): Promise<Release[]> {
    return this.request(`/albums/${id}/search`, { method: 'POST' });
  }

  async downloadAlbum(id: number, data: { release_id?: string; magnet?: string }): Promise<Download> {
    return this.request(`/albums/${id}/download`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  // Downloads endpoints
  async getDownloads(status?: string): Promise<Download[]> {
    const query = status ? `?status=${status}` : '';
    return this.request(`/downloads${query}`);
  }

  async getDownload(id: number): Promise<Download> {
    return this.request(`/downloads/${id}`);
  }

  async pauseDownload(id: number): Promise<Download> {
    return this.request(`/downloads/${id}/pause`, { method: 'POST' });
  }

  async resumeDownload(id: number): Promise<Download> {
    return this.request(`/downloads/${id}/resume`, { method: 'POST' });
  }

  async deleteDownload(id: number, deleteFiles: boolean = false): Promise<{ success: boolean }> {
    return this.request(`/downloads/${id}?delete_files=${deleteFiles}`, {
      method: 'DELETE',
    });
  }

  // Search endpoints
  async searchTmdbMovies(query: string, year?: number): Promise<TmdbMovieSearchResult[]> {
    const searchParams = new URLSearchParams({ q: query });
    if (year) searchParams.append('year', String(year));
    return this.request(`/search/tmdb/movies?${searchParams.toString()}`);
  }

  async searchTmdbTv(query: string): Promise<TmdbTvSearchResult[]> {
    const searchParams = new URLSearchParams({ q: query });
    return this.request(`/search/tmdb/tv?${searchParams.toString()}`);
  }

  async searchMusicBrainzArtists(query: string): Promise<MusicBrainzArtistSearchResult[]> {
    const searchParams = new URLSearchParams({ q: query });
    return this.request(`/search/musicbrainz/artists?${searchParams.toString()}`);
  }

  async searchMusicBrainzAlbums(query: string, artistMbid?: string): Promise<MusicBrainzAlbumSearchResult[]> {
    const searchParams = new URLSearchParams({ q: query });
    if (artistMbid) searchParams.append('artist_mbid', artistMbid);
    return this.request(`/search/musicbrainz/albums?${searchParams.toString()}`);
  }

  // System endpoints
  async getSystemStatus(): Promise<SystemStatus> {
    return this.request('/system/status');
  }

  async getActivity(params?: {
    type?: string;
    limit?: number;
    before?: string;
  }): Promise<ActivityEvent[]> {
    const searchParams = new URLSearchParams();
    if (params?.type) searchParams.append('type', params.type);
    if (params?.limit) searchParams.append('limit', String(params.limit));
    if (params?.before) searchParams.append('before', params.before);

    const query = searchParams.toString();
    return this.request(`/system/activity${query ? `?${query}` : ''}`);
  }
}

// Export singleton instance
export const api = new ApiClient();
