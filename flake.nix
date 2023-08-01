{
  description = "Miscellaneous scripts written in Rust.";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    alejandra = {
      url = "github:kamadorueda/alejandra";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # cargoInt.url = "github:yusdacra/nix-cargo-integration";
    mozilla = {
      url = "github:mozilla/nixpkgs-mozilla";
    };
  };
  outputs = inputs @ {
    self,
    nixpkgs,
    naersk,
    mozilla,
    alejandra,
  }:
    with builtins; let
      std = nixpkgs.lib;
      SYSTEM_NOTIFICATION_ICON = ./assets/pond.svg;
      systems = ["x86_64-linux"];
      genSystems = std.genAttrs systems;
      binaries = let
        files = readDir ./bin;
      in
        std.filter (entry: files.${entry} == "directory") (attrNames files);
      derivations = let
        nlib = final: prev: naersk.lib.${builtins.currentSystem or final.system or "x86_64-linux"};
      in ({
          script-lib = final: prev: let
            nl = nlib final prev;
          in
            nl.buildPackage {
              RUSTFLAGS = [
                "--cfg unsound_local_offset"
              ];
              src = ./.;
              copyLibs = true;
              copyBins = false;
              cargoBuildOptions = base: base ++ ["--all-features"];
              nativeBuildInputs = with final; [pkg-config openssl];
              inherit SYSTEM_NOTIFICATION_ICON;
            };
        }
        // (std.genAttrs binaries (bin: final: prev: let
          nl = nlib final prev;
        in
          nl.buildPackage {
            name = bin;
            RUSTFLAGS = [
              "--cfg unsound_local_offset"
            ];
            src = ./.;
            targets = [bin];
            cargoBuildOptions = base: base ++ ["-p" bin];
            nativeBuildInputs = with final; [pkg-config dbus openssl];
            inherit SYSTEM_NOTIFICATION_ICON;
          })));
    in {
      formatter = std.mapAttrs (system: pkgs: pkgs.default) alejandra.packages;
      overlays.default = final: prev: {
        ash-scripts.rust = std.mapAttrs (name: drv: drv final prev) derivations;
      };
      packages = genSystems (system: let
        pkgs = import nixpkgs {
          localSystem = builtins.currentSystem or system;
          crossSystem = system;
          overlays = [self.overlays.default (self.overlays.${system} or (final: prev: {}))];
        };
      in (std.mapAttrs (name: drv: pkgs.ash-scripts.rust.${name}) derivations));
      apps = std.mapAttrs (system: pkgs:
        std.genAttrs binaries (name: {
          type = "app";
          program = "${pkgs.${name}}/bin/${name}";
        }))
      self.packages;

      homeManagerModules.default = import ./home-manager.nix inputs;
    };
}
