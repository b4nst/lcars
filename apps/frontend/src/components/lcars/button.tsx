import { ButtonHTMLAttributes, forwardRef } from 'react';
import { cn } from '@/lib/utils';

interface LcarsButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'orange' | 'yellow' | 'blue' | 'purple' | 'red';
  size?: 'sm' | 'md' | 'lg';
}

export const LcarsButton = forwardRef<HTMLButtonElement, LcarsButtonProps>(
  ({ className, variant = 'orange', size = 'md', children, ...props }, ref) => {
    const variants = {
      orange: 'bg-lcars-orange hover:bg-lcars-yellow',
      yellow: 'bg-lcars-yellow hover:bg-lcars-orange',
      blue: 'bg-lcars-blue hover:bg-lcars-lavender',
      purple: 'bg-lcars-purple hover:bg-lcars-lavender',
      red: 'bg-lcars-red hover:bg-lcars-orange',
    };

    const sizes = {
      sm: 'px-4 py-1 text-sm',
      md: 'px-6 py-2 text-base',
      lg: 'px-8 py-3 text-lg',
    };

    return (
      <button
        ref={ref}
        type="button"
        className={cn(
          'rounded-lcars font-lcars text-lcars-black uppercase tracking-wider',
          'transition-colors duration-200',
          'focus:outline-none focus:ring-2 focus:ring-lcars-yellow focus:ring-offset-2 focus:ring-offset-lcars-black',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          variants[variant],
          sizes[size],
          className
        )}
        {...props}
      >
        {children}
      </button>
    );
  }
);

LcarsButton.displayName = 'LcarsButton';
