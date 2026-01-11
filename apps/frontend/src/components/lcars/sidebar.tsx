'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { Film, Tv, Music, Download, Settings, Home } from 'lucide-react';
import { cn } from '@/lib/utils';

const navItems = [
  { href: '/', icon: Home, label: 'Dashboard' },
  { href: '/movies', icon: Film, label: 'Movies' },
  { href: '/tv', icon: Tv, label: 'TV Shows' },
  { href: '/music', icon: Music, label: 'Music' },
  { href: '/downloads', icon: Download, label: 'Downloads' },
  { href: '/settings', icon: Settings, label: 'Settings' },
];

export function LcarsSidebar() {
  const pathname = usePathname();

  return (
    <nav className="w-48 flex flex-col gap-1">
      {navItems.map((item) => {
        const isActive =
          pathname === item.href ||
          (item.href !== '/' && pathname.startsWith(item.href));

        return (
          <Link
            key={item.href}
            href={item.href}
            aria-current={isActive ? 'page' : undefined}
            className={cn(
              'flex items-center gap-3 px-4 py-3 rounded-lg transition-colors',
              'focus:outline-none focus:ring-2 focus:ring-lcars-yellow focus:ring-offset-2 focus:ring-offset-lcars-black',
              isActive
                ? 'bg-lcars-orange text-lcars-black'
                : 'bg-lcars-dark text-lcars-text hover:bg-lcars-tan hover:text-lcars-black'
            )}
          >
            <item.icon size={20} aria-hidden="true" />
            <span>{item.label}</span>
          </Link>
        );
      })}
    </nav>
  );
}
