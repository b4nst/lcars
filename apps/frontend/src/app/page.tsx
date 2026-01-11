import { LcarsButton, LcarsPanel } from '@/components/lcars';

export default function Home() {
  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-lcars-text">Dashboard</h1>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <LcarsPanel title="Frontend" accentColor="orange">
          <p className="text-lcars-text">Next.js 14 with Static Export</p>
        </LcarsPanel>

        <LcarsPanel title="Backend" accentColor="blue">
          <p className="text-lcars-text">Rust with Axum</p>
        </LcarsPanel>

        <LcarsPanel title="Build System" accentColor="purple">
          <p className="text-lcars-text">Moon Monorepo</p>
        </LcarsPanel>
      </div>

      <div className="flex gap-4">
        <LcarsButton variant="orange">Orange</LcarsButton>
        <LcarsButton variant="yellow">Yellow</LcarsButton>
        <LcarsButton variant="blue">Blue</LcarsButton>
        <LcarsButton variant="purple">Purple</LcarsButton>
        <LcarsButton variant="red">Red</LcarsButton>
      </div>

      <div className="flex gap-4">
        <LcarsButton size="sm">Small</LcarsButton>
        <LcarsButton size="md">Medium</LcarsButton>
        <LcarsButton size="lg">Large</LcarsButton>
      </div>
    </div>
  );
}
