import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { api } from '../api';
import type { User } from '../types';

/**
 * Authentication store using Zustand with localStorage persistence.
 *
 * Security Note: Token is stored in localStorage for simplicity.
 * For enhanced security in production, consider:
 * - Using httpOnly cookies (requires backend Set-Cookie support)
 * - Implementing Content Security Policy (CSP) headers
 * - Adding token expiration validation
 */

interface AuthState {
  user: User | null;
  token: string | null;
  isLoading: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  checkAuth: () => Promise<void>;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      user: null,
      token: null,
      isLoading: true, // Start as loading until rehydration completes

      login: async (username: string, password: string) => {
        set({ isLoading: true });
        try {
          const { token, user } = await api.login(username, password);
          api.setToken(token);
          set({ user, token, isLoading: false });
        } catch (error) {
          set({ isLoading: false });
          throw error;
        }
      },

      logout: async () => {
        set({ isLoading: true });
        try {
          await api.logout();
        } catch (error) {
          console.error('Logout error:', error);
        } finally {
          api.setToken(null);
          set({ user: null, token: null, isLoading: false });
        }
      },

      checkAuth: async () => {
        const { token } = get();
        if (!token) {
          set({ user: null, isLoading: false });
          return;
        }

        set({ isLoading: true });
        api.setToken(token);

        try {
          const user = await api.me();
          set({ user, isLoading: false });
        } catch (error) {
          console.error('Auth check failed:', error);
          api.setToken(null);
          set({ user: null, token: null, isLoading: false });
        }
      },
    }),
    {
      name: 'lcars-auth',
      partialize: (state) => ({
        token: state.token,
        user: state.user,
      }),
      onRehydrateStorage: () => (state) => {
        // When store rehydrates from localStorage, set the token on the API client
        if (state?.token) {
          api.setToken(state.token);
        }
        // Mark loading as complete after rehydration
        useAuthStore.setState({ isLoading: false });
      },
    }
  )
);
