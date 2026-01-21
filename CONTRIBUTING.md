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


## Monorepo Structure

This project uses [Moon](https://moonrepo.dev/) for monorepo management. Projects are organized as:

- `apps/lcars` - Rust application (backend + HTMX frontend)
- `packages/` - Shared packages

## Development Workflow

### Running in Development Mode

```bash
moon run lcars:dev
```

### Building

```bash
# Build everything
moon run :build

# Build specific project
moon run lcars:build
```

### Testing

```bash
# Run all tests
moon run :test

# lcars tests
moon run lcars:test
```

### Code Quality

```bash
# Format code
moon run lcars:fmt

# Run clippy
moon run lcars:clippy
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
