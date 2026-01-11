import type { Metadata } from 'next';
import { LcarsFrame } from '@/components/lcars';
import { Providers } from '@/lib/providers';
import './globals.css';

export const metadata: Metadata = {
  title: 'LCARS - Media Management',
  description: 'Media management system',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>
        <Providers>
          <LcarsFrame>{children}</LcarsFrame>
        </Providers>
      </body>
    </html>
  );
}
