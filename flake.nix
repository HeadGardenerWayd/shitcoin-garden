{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
  }: let
    system = "x86_64-linux";

    pkgs = import nixpkgs {
      inherit system;
      overlays = [rust-overlay.overlays.default];
    };

    toolchain = pkgs.rust-bin.fromRustupToolchainFile ./toolchain.toml;
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = [
        toolchain
        pkgs.binaryen
        pkgs.cargo-expand
        pkgs.cargo-nextest
        pkgs.cargo-binstall
        pkgs.cargo-tarpaulin
        pkgs.coreutils
        pkgs.just
        pkgs.bun
        pkgs.nodePackages_latest.typescript-language-server
        pkgs.ripgrep
        pkgs.rust-analyzer-unwrapped
        pkgs.tokei
        pkgs.vscode-langservers-extracted
        pkgs.watchexec
      ];

      RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";

      shellHook = ''
        export CARGO_HOME=$(pwd)/.data
        export PATH=$CARGO_HOME/bin:$PATH
      '';
    };
  };

  nixConfig = {
    extra-substituters = [
      "https://nix-community.cachix.org"
    ];

    extra-trusted-public-keys = [
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };
}
