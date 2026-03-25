{
  description = "Bengal Language";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { nixpkgs, rust-overlay, crane, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };

      rustToolchain = pkgs.rust-bin.stable.latest.default;
      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      commonArgs = {
        src = craneLib.cleanCargoSource (craneLib.path ./.);
        strictDeps = true;

        buildInputs = [
          pkgs.openssl
        ];

        nativeBuildInputs = [
          pkgs.pkg-config
        ];

        OPENSSL_NO_VENDOR = 1;
      };

      bengal = craneLib.buildPackage (commonArgs // {
      });
    in
    {
      packages.${system}.default = bengal;

      devShells.${system}.default = pkgs.mkShell {
        inputsFrom = [ bengal ];

        packages = [
          rustToolchain
          pkgs.rust-analyzer
        ];
        
        shellHook = ''
          export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig"
        '';
      };
    };
}