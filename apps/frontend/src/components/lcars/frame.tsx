'use client';

import { LcarsSidebar } from './sidebar';

interface LcarsFrameProps {
  children: React.ReactNode;
}

export function LcarsFrame({ children }: LcarsFrameProps) {
  return (
    <div className="min-h-screen bg-lcars-black flex flex-col">
      {/* Skip to content link for accessibility */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:px-4 focus:py-2 focus:bg-lcars-orange focus:text-lcars-black focus:rounded-lg"
      >
        Skip to content
      </a>

      {/* Top bar */}
      <header className="h-16 flex items-center gap-2 p-2" role="banner">
        <div
          className="bg-lcars-purple rounded-l-lcars-lg h-full w-32"
          aria-hidden="true"
        />
        <div className="bg-lcars-orange h-full flex-1 rounded-r-lg flex items-center px-4">
          <span className="text-lcars-black font-bold text-xl">LCARS</span>
        </div>
      </header>

      <div className="flex-1 flex gap-2 p-2">
        {/* Sidebar */}
        <LcarsSidebar />

        {/* Main content */}
        <main id="main-content" className="flex-1 bg-lcars-dark rounded-lg p-4">
          {children}
        </main>
      </div>

      {/* Bottom bar */}
      <footer className="h-8 flex gap-1 p-2" aria-hidden="true">
        <div className="h-full flex-1 bg-lcars-orange rounded-l-lg" />
        <div className="h-full flex-1 bg-lcars-yellow" />
        <div className="h-full flex-1 bg-lcars-blue" />
        <div className="h-full flex-1 bg-lcars-purple" />
        <div className="h-full flex-1 bg-lcars-peach rounded-r-lg" />
      </footer>
    </div>
  );
}
