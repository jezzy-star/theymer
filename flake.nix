{
  # TODO: figure out how to copy this from Cargo.toml
  description = "full-fat color scheme engine. write once, theme everything.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    crane,
  }: let
    inherit (nixpkgs) lib;
    eachSystem = lib.genAttrs lib.systems.flakeExposed;
    cargo = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    workspace = cargo.workspace.package;
    rustFlags = "-C link-arg=-fuse-ld=mold -Z macro-backtrace";

    pkgsFor = eachSystem (
      system:
        import nixpkgs {
          localSystem.system = system;
          overlays = [(import rust-overlay)];
        }
    );
  in {
    packages = eachSystem (
      system: let
        pkgs = pkgsFor.${system};
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          pname = "theymer";
          version = workspace.version;

          nativeBuildInputs = with pkgs; [
            clang
            mold
            pkg-config
          ];

          buildInputs = with pkgs; [
            openssl
            zlib
          ];

          RUSTFLAGS = rustFlags;

          # disable sccache in nix builds
          CARGO_BUILD_RUSTC_WRAPPER = "";
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        default = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            meta = with lib; {
              description = cargo.package.description;
              homepage = workspace.repository;
              license = workspace.license;
              mainProgram = "theymer";
              platforms = platforms.unix;
            };
          }
        );
      }
    );

    devShells = eachSystem (
      system: let
        pkgs = pkgsFor.${system};
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        pkg = self.packages.${system}.default;
      in {
        default = pkgs.mkShell {
          inputsFrom = [pkg];

          nativeBuildInputs = [
            pkgs.alejandra
            pkgs.sccache
            rustToolchain
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          RUSTFLAGS = rustFlags;
          RUST_BACKTRACE = "1";

          shellHook = ''
            ${lib.optionalString pkgs.stdenv.isLinux ''
              export LD_LIBRARY_PATH="${lib.makeLibraryPath pkg.buildInputs}:''${LD_LIBRARY_PATH:-}"
            ''}

            ${lib.optionalString pkgs.stdenv.isDarwin ''
              export DYLD_LIBRARY_PATH="${lib.makeLibraryPath pkg.buildInputs}:''${DYLD_LIBRARY_PATH:-}"
            ''}

             echo ""
             echo "theymer developer environment active!"
             echo "using $(rustc --version) on ${system}"
             echo ""
          '';
        };
      }
    );

    formatter = eachSystem (system: pkgsFor.${system}.alejandra);
  };
}
