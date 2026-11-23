{
  description = "DIYPets - Lightweight (~7.8 MB RAM) cross-platform Rust desktop mascot engine, 2D/SVG Studio & MCP server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          isLinux = pkgs.stdenv.isLinux;
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "diypets";
            version = "0.4.1";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = pkgs.lib.optionals isLinux [ pkgs.pkg-config ];
            buildInputs = pkgs.lib.optionals isLinux [
              pkgs.gtk3
              pkgs.libayatana-appindicator
              pkgs.xdotool
              pkgs.xorg.libxcb
              pkgs.libxkbcommon
              pkgs.wayland
              pkgs.libGL
            ];
            doCheck = false;
            meta = with pkgs.lib; {
              description = "Cross-platform desktop mascot engine in Rust with 2D/SVG Studio and MCP server";
              homepage = "https://shivammahajan.com/diypets/";
              license = with licenses; [ mit asl20 ];
              mainProgram = "diypets";
            };
          };
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/diypets";
        };
      });
    };
}
