{
  description = "Rust nightly for smx";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { nixpkgs, rust-overlay, ...}:
    let
      system = "x86_64-linux";

      overlays = [
        (import rust-overlay)
      ];

      pkgs = import nixpkgs {
        inherit system overlays;
      };

      rustToolchain = pkgs.rust-bin.nightly.latest.default.override {
        extensions = [
          "rust-analyzer"
          "clippy"
          "rust-src"
          "rustfmt"
        ];
      };
    in
      {
        devShells.${system}.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain

            pkg-config
            openssl
            cargo-watch
            cargo-nextest
          ];
        };
      };
}
