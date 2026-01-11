/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        lcars: {
          orange: 'var(--lcars-orange)',
          yellow: 'var(--lcars-yellow)',
          blue: 'var(--lcars-blue)',
          purple: 'var(--lcars-purple)',
          red: 'var(--lcars-red)',
          peach: 'var(--lcars-peach)',
          tan: 'var(--lcars-tan)',
          lavender: 'var(--lcars-lavender)',
          black: 'var(--lcars-black)',
          dark: 'var(--lcars-dark)',
          text: 'var(--lcars-text)',
          'text-dim': 'var(--lcars-text-dim)',
        },
        status: {
          available: 'var(--status-available)',
          missing: 'var(--status-missing)',
          downloading: 'var(--status-downloading)',
          processing: 'var(--status-processing)',
        },
      },
      fontFamily: {
        lcars: ['Antonio', 'Helvetica Neue', 'sans-serif'],
      },
      borderRadius: {
        lcars: '1.5rem',
        'lcars-lg': '2.5rem',
      },
    },
  },
  plugins: [],
};
