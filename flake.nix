{
  description = "Miscellaneous scripts written in Rust.";
  inputs = {
    nixpkgs.url = github:nixos/nixpkgs/nixpkgs-unstable;
    naersk.url = github:nix-community/naersk;
    # cargoInt.url = github:yusdacra/nix-cargo-integration;
    mozilla = {
      url = github:mozilla/nixpkgs-mozilla;
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { self, nixpkgs, naersk, mozilla }:
    let
      std = nixpkgs.lib;
      systems = [ "x86_64-linux" ];
      genSystems = std.genAttrs systems;
      derivations = std.genAttrs systems
        (system: {
          ash-scripts = final: prev:
            let
              rust = ((mozilla.overlays.rust final prev).rustChannelOf {
                date = "2022-04-05";
                channel = "nightly";
                sha256 = "0eyEJlGQbev/oZUw5LbRcddkUvjyKSLEHdxWJiOOA/k=";
              }).rust;
              nlib = naersk.lib.${system}.override {
                cargo = rust;
                rustc = rust;
              };
            in
            nlib.buildPackage {
              name = "ash-scripts";
              RUSTFLAGS = [
                "--cfg unsound_local_offset"
              ];
              src = ./.;
              nativeBuildInputs = with final; [ pkg-config dbus openssl ];
            };
        });
    in
    {
      overlays = genSystems (system: final: prev: std.mapAttrs (name: drv: drv final prev) derivations.${system});
      packages = genSystems (system:
        let
          pkgs = import nixpkgs { inherit system; overlays = [ self.overlays.${system} ]; };
        in
        std.mapAttrs (name: drv: pkgs.${name}) derivations.${system});
      defaultPackage = genSystems (system: self.packages.${system}.ash-scripts);
    };
}
