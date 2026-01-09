{
  description = "LCARS - Media Management Monorepo";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    t3rapkgs.url = "github:t3ra-oss/t3rapkgs";
  };

  outputs = { self, nixpkgs, flake-utils, t3rapkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ t3rapkgs.overlays.default ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer

            # Bun for frontend
            bun

            # Moon for monorepo management
            moonrepo

            # Other development tools
            git
            pkg-config
            openssl
          ];

          shellHook = ''
            echo "🚀 LCARS Development Environment"
            echo "  - Rust: $(rustc --version)"
            echo "  - Bun: $(bun --version)"
            echo "  - Moon: $(moon --version)"
          '';
        };

        apps.default = {
          type = "app";
          program = "${pkgs.writeShellScript "moon-ci" ''
            export PATH="${pkgs.lib.makeBinPath (with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              bun
              moonrepo
              git
              pkg-config
              openssl
            ])}"
            exec ${pkgs.moonrepo}/bin/moon ci
          ''}";
        };
      }
    );
}
