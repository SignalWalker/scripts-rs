{
  description = "Miscellaneous scripts written in Rust.";
  inputs = {
    nixpkgs.url = github:nixos/nixpkgs/nixpkgs-unstable;
    naersk = {
      url = github:nix-community/naersk;
      inputs.nixpkgs.follows = "nixpkgs";
    };
    alejandra = {
      url = github:kamadorueda/alejandra;
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # cargoInt.url = github:yusdacra/nix-cargo-integration;
    mozilla = {
      url = github:mozilla/nixpkgs-mozilla;
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = {
    self,
    nixpkgs,
    naersk,
    mozilla,
    alejandra,
  }:
    with builtins; let
      std = nixpkgs.lib;
      systems = ["x86_64-linux"];
      genSystems = std.genAttrs systems;
      binaries = let
        files = readDir ./bin;
      in
        std.filter (entry: files.${entry} == "directory") (attrNames files);
      derivations = let
        toolchain = final: prev: ((mozilla.overlays.rust final prev).rustChannelOf {
          date = "2022-05-04";
          channel = "nightly";
          sha256 = "0eyEJlGQbev/oZUw5LbRcddkUvjyKSLEHdxWJiOOA/k=";
        });
        nlib = final: prev: let
          tc = toolchain final prev;
        in
          naersk.lib.${builtins.currentSystem or "x86_64-linux"}.override {
            cargo = tc.rust;
            rustc = tc.rust;
          };
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
    };
}
