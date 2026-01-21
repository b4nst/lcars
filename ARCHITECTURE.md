# LCARS Monorepo Boilerplate

This document describes the monorepo setup created for the LCARS project.

## Overview

The LCARS project is set up as a monorepo using [Moon](https://moonrepo.dev/) for build orchestration, with:
- **LCARS App**: Rust service using Axum framework with HTMX frontend
- **Development Environment**: Nix flake with devshell using t3rapkgs

## Directory Structure

```
lcars/
├── .github/
│   └── workflows/
│       └── ci.yml              # GitHub Actions CI pipeline
├── .moon/
│   ├── toolchain.yml           # Moon toolchain configuration
│   └── workspace.yml           # Moon workspace configuration
├── apps/
│   └── lcars/                  # Rust application (backend + HTMX frontend)
│       ├── src/
│       │   └── main.rs         # Axum server
│       ├── templates/          # Askama HTML templates
│       ├── static/             # Static assets (CSS, JS)
│       ├── Cargo.toml          # Rust dependencies
│       └── moon.yml            # Moon tasks
├── packages/                   # Shared packages
│   └── soulseek-protocol/      # Soulseek protocol implementation
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

### 2. LCARS Application (Rust + Axum + HTMX)

**Location**: `apps/lcars/`

**Technologies**:
- Rust 2021 edition
- Axum 0.7 web framework
- Tokio async runtime
- Serde for serialization
- Askama for HTML templates
- HTMX for dynamic frontend

**Moon Tasks** (`moon.yml`):
- `build`: Cargo release build
- `dev`: Run with cargo run
- `check`: Cargo check
- `test`: Run tests
- `fmt`: Format checking
- `clippy`: Linting

### 3. Nix Development Environment

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
- Provides `apps.default` that exposes a shell environment for running commands

### 4. CI/CD

**GitHub Actions** (`.github/workflows/ci.yml`):

Single CI job using Nix and Moon:
1. Checkout code
2. Install Nix
3. Run `nix run .#default -- moon ci` to execute moon ci in the flake's environment
   - Flake app provides shell with all required dependencies
   - Executes all configured tasks (build, test, lint, etc.)
   - Unified workflow managed by Moon

## Usage

### First Time Setup

```bash
# Enter Nix shell
nix develop
```

### Development

```bash
moon run lcars:dev
```

### Building

```bash
# Build everything
moon run :build

# Or individually
moon run lcars:build
```

### Testing

```bash
# Run all tests
moon run :test

# lcars tests only
moon run lcars:test
```

### Code Quality

```bash
moon run lcars:fmt      # Format check
moon run lcars:clippy   # Linting
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
