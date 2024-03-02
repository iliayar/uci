{
  description = "TODO";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
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

        buildRustPackage = pkgs.rustPlatform.buildRustPackage.override {
          rustc = pkgs.rust-bin.stable.latest.default;
        };
      in {
        devShell = pkgs.mkShell rec {
          buildInputs = with pkgs;
            [
              rust-bin.stable.latest.default
              rust-analyzer
              rustfmt

              pkgconfig
              openssl

              caddy

              # Other packages
            ] ++ (if system == "x86_64-darwin" then
              [ pkgs.darwin.apple_sdk.frameworks.Security ]
            else
              [ ]);
        };

        packages = rec {
          ucid = buildRustPackage {
            pname = "ucid";
            version = "1.0.0";
            src = ./.;
            cargoBuildFlags = "-p ucid";

            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [ openssl stdenv.cc.cc.libgcc ];
            cargoLock = { lockFile = ./Cargo.lock; };
          };

          uci = buildRustPackage {
            pname = "uci";
            version = "1.0.0";
            src = ./.;
            cargoBuildFlags = "-p uci";

            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [ openssl stdenv.cc.cc.libgcc ];
            cargoLock = { lockFile = ./Cargo.lock; };
          };

          default = uci;
        };
      });
}
