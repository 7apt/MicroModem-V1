{
  description = "MicroModem development and release environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "armv7l-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
        default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          rustc
          pkg-config
          libGL libxkbcommon wayland
          libx11 libxi libxcursor libxrandr
        ];

        shellHook = ''
          export MICROMODEM_DEV_SHELL=1
          export LD_LIBRARY_PATH="${nixpkgs.lib.makeLibraryPath (with pkgs; [ libGL libxkbcommon wayland libx11 libxi libxcursor libxrandr ])}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          echo "MicroModem dev shell — cargo $(cargo --version | cut -d' ' -f2)"
        '';
        };
      });

      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          version = "0.1.0";
        in rec {
          release = pkgs.runCommand "micromodem-server-${version}" {
            nativeBuildInputs = [ pkgs.gnutar pkgs.gzip ];
          } ''
            release_root="$TMPDIR/micromodem-server-${version}"
            mkdir -p "$release_root/gateway/downstream" "$out"
            cp ${./gateway/compose.yml} "$release_root/gateway/compose.yml"
            cp ${./gateway/.env.example} "$release_root/gateway/.env.example"
            cp ${./gateway/micromodem-gateway} "$release_root/gateway/micromodem-gateway"
            cp ${./gateway/downstream/Dockerfile} "$release_root/gateway/downstream/Dockerfile"
            cp ${./gateway/downstream/entrypoint.sh} "$release_root/gateway/downstream/entrypoint.sh"
            cp ${./gateway/release-launcher} "$release_root/micromodem"
            cp ${./gateway/RELEASE.md} "$release_root/README.md"
            chmod 0755 "$release_root/micromodem" "$release_root/gateway/micromodem-gateway" \
              "$release_root/gateway/downstream/entrypoint.sh"
            tar -C "$TMPDIR" -czf "$out/micromodem-server-${version}.tar.gz" \
              "micromodem-server-${version}"
            sha256sum "$out/micromodem-server-${version}.tar.gz" \
              > "$out/micromodem-server-${version}.tar.gz.sha256"
          '';
          default = release;
        });
    };
}
