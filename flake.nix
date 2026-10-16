{
  description = "Laneboard — agent-friendly kanban CLI (`board`)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Match rust-toolchain.toml channel (skip the Windows cross target).
        rustToolchain = pkgs.rust-bin.stable."1.99.0".default.override {
          extensions = [
            "rust-src"
            "clippy"
            "rustfmt"
          ];
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        board = pkgs.callPackage ./nix/package.nix {
          inherit rustPlatform;
        };
      in
      {
        packages = {
          default = board;
          board = board;
        };

        apps.default = {
          type = "app";
          program = "${board}/bin/board";
        };

        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.pkg-config
            pkgs.cargo-watch
          ];
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
