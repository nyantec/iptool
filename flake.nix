{
  description = "iptool";

  outputs = { self, nixpkgs }: let
    version = self.shortRev or (toString self.lastModifiedDate);
    overlay = final: prev: {
      iptool = final.callPackage (
        { rustPlatform }: rustPlatform.buildRustPackage {
          pname = "iptool";
          inherit version;
          src = self;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "--all-features" ];
        }
      ) {};
      iptool-tests = final.iptool.overrideAttrs (old: {
        name = "iptool-test-binaries";
        nativeBuildInputs = (old.nativeBuildInputs or []) ++ [ final.perl ];

        cargoBuildFlags = (old.cargoBuildFlags or []) ++ [ "--tests" ];

        installPhase = ''
          find target -type f -executable -regex ".*-[0-9a-f]+" \
            | sed 's#\(.*\)/\([^/]*\)-\([0-9a-f]\+\)#install -Dm755 \1/\2-\3 \$out/bin/\2-test#' \
            | sh
        '';
      });

    };
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);

    nixpkgsFor = forAllSystems (system:
      import nixpkgs {
        inherit system;
        overlays = [ overlay ];
    });
  in {
    inherit overlay;
    packages = forAllSystems (system: { inherit (nixpkgsFor.${system}) iptool iptool-tests; });

    defaultPackage = forAllSystems (system: self.packages.${system}.iptool);
  };
}
