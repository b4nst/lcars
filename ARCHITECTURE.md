# LCARS Monorepo Boilerplate

This document describes the monorepo setup created for the LCARS project.

## Overview

The LCARS project is set up as a monorepo using [Moon](https://moonrepo.dev/) for build orchestration, with:
- **Backend**: Rust service using the Axum framework
- **Frontend**: Next.js 14 SPA with static export capabilities (using Bun)
- **Package Manager**: Bun for the frontend
- **Development Environment**: Nix flake with devshell using t3rapkgs

## Directory Structure

```
lcars/
├── .github/
│   └── workflows/
│       └── ci.yml              # GitHub Actions CI pipeline
├── .moon/
│   ├── toolchain.yml           # Moon toolchain configuration (Rust + Bun)
│   └── workspace.yml           # Moon workspace configuration
├── apps/
│   ├── backend/                # Rust backend application
│   │   ├── src/
│   │   │   └── main.rs         # Axum server with health check endpoint
│   │   ├── Cargo.toml          # Rust dependencies
│   │   └── moon.yml            # Moon tasks for backend
│   └── frontend/               # Next.js frontend application
│       ├── src/
│       │   └── app/
│       │       ├── globals.css # Global styles with Tailwind
│       │       ├── layout.tsx  # Root layout
│       │       └── page.tsx    # Home page
│       ├── public/             # Static assets
│       ├── package.json        # Node dependencies
│       ├── next.config.js      # Next.js config with static export
│       ├── tsconfig.json       # TypeScript configuration
│       ├── tailwind.config.js  # Tailwind CSS configuration
│       ├── postcss.config.js   # PostCSS configuration
│       ├── .eslintrc.js        # ESLint configuration
│       └── moon.yml            # Moon tasks for frontend
├── packages/                   # Future shared packages
│   └── .gitkeep
├── .envrc                      # direnv configuration
├── .gitattributes              # Git attributes
├── .gitignore                  # Git ignore patterns
├── CONTRIBUTING.md             # Contribution guidelines
├── flake.nix                   # Nix flake for dev environment
├── LICENSE                     # MIT License
└── README.md                   # Project README
```

## Key Features

### 1. Moon Workspace Configuration

**File**: `.moon/workspace.yml`
- Configured to automatically discover projects in `apps/*` and `packages/*`
- Git VCS integration with `main` as default branch

**File**: `.moon/toolchain.yml`
- Bun 1.0.0 for frontend development
- Rust 1.75.0 for backend development
- Uses official Moon plugins for both languages

### 2. Backend (Rust + Axum)

**Location**: `apps/backend/`

**Technologies**:
- Rust 2021 edition
- Axum 0.7 web framework
- Tokio async runtime
- Serde for serialization

**Moon Tasks** (`moon.yml`):
- `build`: Cargo release build
- `dev`: Run with cargo run
- `check`: Cargo check
- `test`: Run tests
- `fmt`: Format checking
- `clippy`: Linting

**Sample Endpoint**: 
- `GET /health` - Returns JSON with message and version

### 3. Frontend (Next.js + Bun)

**Location**: `apps/frontend/`

**Technologies**:
- Next.js 14 with App Router
- React 18
- TypeScript 5
- Tailwind CSS 3
- Static export mode

**Moon Tasks** (`moon.yml`):
- `dev`: Development server
- `build`: Static build and export
- `lint`: ESLint
- `typecheck`: TypeScript checking

**Features**:
- Static export enabled (`output: 'export'`)
- Tailwind CSS for styling
- TypeScript for type safety
- App Router structure

### 4. Nix Development Environment

**File**: `flake.nix`

**Provided Tools**:
- Rust toolchain (rustc, cargo, rustfmt, clippy, rust-analyzer)
- Bun
- Moon (moonrepo)
- Git
- pkg-config, openssl

**Integration**:
- Uses t3rapkgs overlay
- Works with direnv via `.envrc`
- Displays version info on shell entry

### 5. CI/CD

**GitHub Actions** (`.github/workflows/ci.yml`):

Single CI job using Nix and Moon:
1. Checkout code
2. Install Nix
3. Run `moon ci` within Nix development shell
   - Executes all configured tasks (build, test, lint, etc.)
   - Unified workflow managed by Moon

## Usage

### First Time Setup

```bash
# Enter Nix shell
nix develop

# Install frontend dependencies
cd apps/frontend && bun install
```

### Development

```bash
# Backend
moon run backend:dev

# Frontend (in another terminal)
moon run frontend:dev
```

### Building

```bash
# Build everything
moon run :build

# Or individually
moon run backend:build
moon run frontend:build
```

### Testing

```bash
# Run all tests
moon run :test

# Backend tests only
moon run backend:test
```

### Code Quality

```bash
# Backend
moon run backend:fmt      # Format check
moon run backend:clippy   # Linting

# Frontend
moon run frontend:lint       # ESLint
moon run frontend:typecheck  # TypeScript
```

## Next Steps

This boilerplate provides a solid foundation. Consider:

1. **Add shared packages**: Create shared libraries in `packages/`
2. **Database integration**: Add database support to backend
3. **API client**: Create typed API client in frontend
4. **Authentication**: Implement auth flow
5. **Testing**: Add comprehensive test suites
6. **Docker**: Add Dockerfiles for deployment
7. **Documentation**: Expand API and component docs

## Resources

- [Moon Documentation](https://moonrepo.dev/docs)
- [Axum Framework](https://docs.rs/axum)
- [Next.js Documentation](https://nextjs.org/docs)
- [Bun Documentation](https://bun.sh/docs)
- [Nix Flakes](https://nixos.wiki/wiki/Flakes)
- [t3rapkgs](https://github.com/t3ra-oss/t3rapkgs)
