{
  description = "DevShell: Rust + Bun + Tauri (Linux)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Toolchains
            rustup
            nodejs_20
            bun
            cargo-tauri

            # Tauri Linux deps (WebKitGTK backend)
            pkg-config
            openssl
            webkitgtk
            gtk3
            cairo
            pango
            gdk-pixbuf
            glib
            libsoup_3
            at-spi2-atk
          ];

          shellHook = ''
            # Ensure a Rust toolchain is selected for this shell
            if ! command -v cargo >/dev/null 2>&1; then
              rustup default stable >/dev/null
            fi
            echo "✅ Devshell ready: rustup(stable) + node20 + bun + tauri deps"
          '';
        };
      });
}
