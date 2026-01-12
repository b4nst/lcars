'use client';

import { useState } from 'react';
import { useAuthStore } from '@/lib/stores/auth';
import { LcarsButton } from './button';
import { cn } from '@/lib/utils';

export function LcarsLoginForm() {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const { login, isLoading } = useAuthStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    try {
      await login(username, password);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed');
    }
  };

  return (
    <div className="min-h-screen bg-lcars-black flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        {/* LCARS-style header */}
        <div className="flex items-center gap-2 mb-8">
          <div className="bg-lcars-purple rounded-l-lcars-lg h-12 w-24" />
          <div className="bg-lcars-orange h-12 flex-1 rounded-r-lg flex items-center px-4">
            <span className="text-lcars-black font-bold text-xl">LCARS</span>
          </div>
        </div>

        {/* Login panel */}
        <div className="flex">
          {/* Left accent bar */}
          <div className="w-3 bg-lcars-orange rounded-l-lcars" />

          {/* Form content */}
          <div className="flex-1 bg-lcars-dark rounded-r-lg p-6">
            <h1 className="text-lcars-text text-2xl mb-6">System Access</h1>

            <form onSubmit={handleSubmit} className="space-y-4">
              <div>
                <label
                  htmlFor="username"
                  className="block text-lcars-text-dim text-sm mb-1"
                >
                  Username
                </label>
                <input
                  id="username"
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  className={cn(
                    'w-full px-4 py-2 rounded-lg',
                    'bg-lcars-black border-2 border-lcars-orange',
                    'text-lcars-text placeholder-lcars-text-dim',
                    'focus:outline-none focus:border-lcars-yellow',
                    'transition-colors'
                  )}
                  placeholder="Enter username"
                  required
                  autoComplete="username"
                  disabled={isLoading}
                />
              </div>

              <div>
                <label
                  htmlFor="password"
                  className="block text-lcars-text-dim text-sm mb-1"
                >
                  Password
                </label>
                <input
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className={cn(
                    'w-full px-4 py-2 rounded-lg',
                    'bg-lcars-black border-2 border-lcars-orange',
                    'text-lcars-text placeholder-lcars-text-dim',
                    'focus:outline-none focus:border-lcars-yellow',
                    'transition-colors'
                  )}
                  placeholder="Enter password"
                  required
                  autoComplete="current-password"
                  disabled={isLoading}
                />
              </div>

              {error && (
                <div className="bg-lcars-red/20 border border-lcars-red rounded-lg px-4 py-2">
                  <p className="text-lcars-red text-sm">{error}</p>
                </div>
              )}

              <LcarsButton
                type="submit"
                variant="orange"
                size="lg"
                className="w-full mt-6"
                disabled={isLoading}
              >
                {isLoading ? 'Authenticating...' : 'Login'}
              </LcarsButton>
            </form>
          </div>
        </div>

        {/* Bottom decorative bar */}
        <div className="flex gap-1 mt-4 h-2">
          <div className="flex-1 bg-lcars-orange rounded-l-lg" />
          <div className="flex-1 bg-lcars-yellow" />
          <div className="flex-1 bg-lcars-blue" />
          <div className="flex-1 bg-lcars-purple rounded-r-lg" />
        </div>
      </div>
    </div>
  );
}
