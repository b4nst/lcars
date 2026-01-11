'use client';

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useAuthStore } from './stores/auth';
import { useDownloadsStore } from './stores/downloads';

export function Providers({ children }: { children: React.ReactNode }) {
  // Create QueryClient instance per component to avoid sharing state between requests
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 30000, // 30 seconds
            retry: 1,
            refetchOnWindowFocus: false,
          },
        },
      })
  );

  const checkAuth = useAuthStore((state) => state.checkAuth);
  const connectWs = useDownloadsStore((state) => state.connect);
  const disconnectWs = useDownloadsStore((state) => state.disconnect);

  useEffect(() => {
    // Initialize auth first, then connect WebSocket
    // This ensures token is available for WebSocket authentication
    const initializeApp = async () => {
      await checkAuth();
      connectWs();
    };

    initializeApp();

    // Cleanup WebSocket on unmount
    return () => {
      disconnectWs();
    };
  }, [checkAuth, connectWs, disconnectWs]);

  // Always wrap children in QueryClientProvider
  // This ensures QueryClient is available during SSR/pre-rendering
  return (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
}
