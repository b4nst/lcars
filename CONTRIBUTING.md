# Contributing to LCARS

Thank you for your interest in contributing to LCARS!

## Development Setup

### Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- [direnv](https://direnv.net/) (optional but recommended)

### Getting Started

1. Clone the repository:
   ```bash
   git clone https://github.com/b4nst/lcars.git
   cd lcars
   ```

2. Enter the development environment:
   ```bash
   nix develop
   ```
   
   Or with direnv:
   ```bash
   direnv allow
   ```

3. Install frontend dependencies:
   ```bash
   cd apps/frontend && bun install
   ```

## Monorepo Structure

This project uses [Moon](https://moonrepo.dev/) for monorepo management. Projects are organized as:

- `apps/backend` - Rust backend service
- `apps/frontend` - Next.js frontend application
- `packages/` - Shared packages (future)

## Development Workflow

### Running in Development Mode

```bash
# Backend
moon run backend:dev

# Frontend
moon run frontend:dev
```

### Building

```bash
# Build everything
moon run :build

# Build specific project
moon run backend:build
moon run frontend:build
```

### Testing

```bash
# Run all tests
moon run :test

# Backend tests
moon run backend:test
```

### Code Quality

```bash
# Format backend code
moon run backend:fmt

# Run clippy on backend
moon run backend:clippy

# Lint frontend
moon run frontend:lint

# Type check frontend
moon run frontend:typecheck
```

### Moon Commands

Moon provides powerful commands for working with the monorepo:

```bash
# List all projects
moon project list

# Run a task across all projects
moon run :build

# Check project configuration
moon check --all

# View task graph
moon task graph backend:build
```

## Submitting Changes

1. Create a new branch for your feature/fix
2. Make your changes
3. Run tests and linting
4. Commit your changes with clear commit messages
5. Push to your fork and submit a pull request

## Code Style

- **Rust**: Follow standard Rust formatting (use `cargo fmt`)
- **TypeScript/JavaScript**: Follow Next.js conventions
- Write clear, descriptive commit messages
- Keep PRs focused on a single feature or fix

## Questions?

Feel free to open an issue for any questions or concerns!
