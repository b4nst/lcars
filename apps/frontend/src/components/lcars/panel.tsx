import { cn } from '@/lib/utils';

interface LcarsPanelProps {
  title?: string;
  accentColor?: 'orange' | 'yellow' | 'blue' | 'purple';
  children: React.ReactNode;
  className?: string;
}

export function LcarsPanel({
  title,
  accentColor = 'orange',
  children,
  className,
}: LcarsPanelProps) {
  const colors = {
    orange: 'bg-lcars-orange',
    yellow: 'bg-lcars-yellow',
    blue: 'bg-lcars-blue',
    purple: 'bg-lcars-purple',
  };

  return (
    <div className={cn('flex', className)}>
      {/* Accent bar */}
      <div className={cn('w-2 rounded-l-lcars', colors[accentColor])} />

      {/* Content */}
      <div className="flex-1 bg-lcars-dark rounded-r-lg p-4">
        {title && (
          <h3 className="text-lcars-text-dim text-sm mb-2">{title}</h3>
        )}
        {children}
      </div>
    </div>
  );
}
