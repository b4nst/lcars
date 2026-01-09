{
  description = "LCARS - Media Management Monorepo";
  
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devshell.url = "github:numtide/devshell";
    t3rapkgs = {
      url = "github:t3ra-oss/t3rapkgs";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };
  
  outputs =
    {
      self,
      flake-utils,
      devshell,
      nixpkgs,
      t3rapkgs,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            devshell.overlays.default
            t3rapkgs.overlays.default
          ];
        };
        # Create dev shells using t3ra pkgs
        shells = t3rapkgs.lib.devshell.mkDevShells {
          inherit pkgs system;
          name = "lcars";
          packages = with pkgs; [
            # Rust toolchain
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer

            # Bun for frontend
            bun

            # Other development tools
            git
            pkg-config
            openssl
          ];
          monorepo = true;
        };
      in
      {
        # Dev shells
        devShells = shells.devShells;
        # Apps
        apps = shells.apps;
      });
}
