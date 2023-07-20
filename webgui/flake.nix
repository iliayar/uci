{
  description = "TODO";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
      in {
        devShell = pkgs.mkShell rec {
          buildInputs = with pkgs;
            [
              (rust-bin.nightly.latest.default.override {
                targets = [ "wasm32-unknown-unknown" ];
              })
              rust-analyzer
              rustfmt
              trunk
              # cargo-leptos

              nodePackages.prettier
              tailwindcss

              caddy
            ];
        };
      });
}
