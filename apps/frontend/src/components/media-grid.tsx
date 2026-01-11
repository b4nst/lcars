import { forwardRef, HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface MediaGridProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
}

export const MediaGrid = forwardRef<HTMLDivElement, MediaGridProps>(
  ({ children, className, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          'grid gap-4',
          // Responsive columns: 2 on mobile, 3 on sm, 4 on md, 5 on lg, 6 on xl
          'grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6',
          className
        )}
        {...props}
      >
        {children}
      </div>
    );
  }
);

MediaGrid.displayName = 'MediaGrid';
