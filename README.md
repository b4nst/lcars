# LCARS

Media management system built as a monorepo with Moon.

## Architecture

- **Backend**: Rust with Axum framework (`apps/backend`)
- **Frontend**: Next.js 14 SPA with static export using Bun (`apps/frontend`)
- **Build System**: Moon monorepo manager
- **Dev Environment**: Nix flake with devshell using t3rapkgs

## Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- [direnv](https://direnv.net/) (optional but recommended)

## Getting Started

### Setup Development Environment

1. Clone the repository:
   ```bash
   git clone https://github.com/b4nst/lcars.git
   cd lcars
   ```

2. Enter the Nix development shell:
   ```bash
   nix develop
   ```

   Or if using direnv:
   ```bash
   direnv allow
   ```

### Working with the Monorepo

This project uses [Moon](https://moonrepo.dev/) for monorepo management.

#### Install Dependencies

```bash
# Install frontend dependencies
cd apps/frontend && bun install
```

#### Run Development Servers

```bash
# Run backend
moon run backend:dev

# Run frontend
moon run frontend:dev
```

#### Build Projects

```bash
# Build backend
moon run backend:build

# Build frontend (static export)
moon run frontend:build
```

#### Run Tests and Checks

```bash
# Backend tests
moon run backend:test

# Backend linting
moon run backend:fmt
moon run backend:clippy

# Frontend linting
moon run frontend:lint

# Frontend type checking
moon run frontend:typecheck
```

## Project Structure

```
.
├── .moon/              # Moon configuration
│   ├── workspace.yml   # Workspace settings
│   └── toolchain.yml   # Toolchain configuration
├── apps/
│   ├── backend/        # Rust backend service
│   │   ├── src/
│   │   ├── Cargo.toml
│   │   └── moon.yml
│   └── frontend/       # Next.js frontend SPA
│       ├── src/
│       ├── package.json
│       └── moon.yml
├── packages/           # Shared packages (future)
├── flake.nix           # Nix flake configuration
└── .envrc              # direnv configuration
```

## Development Tools

The Nix devshell provides:
- Rust toolchain (rustc, cargo, rustfmt, clippy, rust-analyzer)
- Bun for frontend development
- Moon for monorepo management
- Additional development utilities

## License

TBD
