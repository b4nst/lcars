'use client';

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useState, useRef } from 'react';
import { useAuthStore } from './stores/auth';
import { useDownloadsStore } from './stores/downloads';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000, // 30 seconds
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

export function Providers({ children }: { children: React.ReactNode }) {
  const [mounted, setMounted] = useState(false);
  const initRef = useRef(false);
  const checkAuth = useAuthStore((state) => state.checkAuth);
  const connectWs = useDownloadsStore((state) => state.connect);
  const disconnectWs = useDownloadsStore((state) => state.disconnect);

  useEffect(() => {
    setMounted(true);

    // Prevent double initialization in strict mode
    if (initRef.current) return;
    initRef.current = true;

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

  // Render children immediately to avoid layout shift
  // Zustand handles SSR hydration automatically
  if (!mounted) {
    return <>{children}</>;
  }

  return (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
}
