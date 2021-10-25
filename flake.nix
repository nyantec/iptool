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
    };
    pkgs = import nixpkgs {
      system = "x86_64-linux";
      overlays = [ overlay ];
    };
  in {
    inherit overlay;
    packages.x86_64-linux = {
      inherit (pkgs) iptool;
    };
    defaultPackage.x86_64-linux = self.packages.x86_64-linux.iptool;
  };
}
