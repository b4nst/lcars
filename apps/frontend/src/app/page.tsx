export default function Home() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center p-24">
      <div className="z-10 max-w-5xl w-full items-center justify-between font-mono text-sm">
        <h1 className="text-4xl font-bold mb-4">LCARS</h1>
        <p className="text-xl mb-8">Media Management System</p>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <div className="rounded-lg border border-gray-300 p-6">
            <h2 className="text-lg font-semibold mb-2">Frontend</h2>
            <p className="text-sm text-gray-600">Next.js 14 with Static Export</p>
          </div>
          <div className="rounded-lg border border-gray-300 p-6">
            <h2 className="text-lg font-semibold mb-2">Backend</h2>
            <p className="text-sm text-gray-600">Rust with Axum</p>
          </div>
          <div className="rounded-lg border border-gray-300 p-6">
            <h2 className="text-lg font-semibold mb-2">Build System</h2>
            <p className="text-sm text-gray-600">Moon Monorepo</p>
          </div>
        </div>
      </div>
    </main>
  )
}
