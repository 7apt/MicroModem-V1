{
  description = "MicroModem native Rust GUI toolchain";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
    in {
      devShells = nixpkgs.lib.genAttrs systems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo rustc pkg-config libGL libxkbcommon wayland
              libx11 libxi libxcursor libxrandr
            ];
            shellHook = ''
              export MICROMODEM_DEV_SHELL=1
              export LD_LIBRARY_PATH="${nixpkgs.lib.makeLibraryPath (with pkgs; [ libGL libxkbcommon wayland libx11 libxi libxcursor libxrandr ])}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              echo "MicroModem dev shell — cargo $(cargo --version | cut -d' ' -f2)"
            '';
          };
        });
    };
}
