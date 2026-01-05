{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";

    # crane = {
    #   url = "github:ipetkov/crane";
    #   inputs.nixpkgs.follows = "nixpkgs";
    # };

    systems.url = "github:nix-systems/default";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    systems,
    nixpkgs,
    treefmt-nix,
    ...
  } @ inputs: let
    eachSystem = f:
      nixpkgs.lib.genAttrs (import systems) (
        system:
          f (import nixpkgs {
            inherit system;
            overlays = [inputs.rust-overlay.overlays.default];
          })
      );

    rustToolchain = eachSystem (pkgs: (pkgs.rust-bin.stable.latest.default.override {
      extensions = ["rust-src"];
    }));

    nightly = eachSystem (pkgs:
      pkgs.rust-bin.selectLatestNightlyWith (t:
        t.default.override {
          extensions = ["rust-docs-json"];
        }));

    cargo-expand' = eachSystem (pkgs: let
      nightly' = nightly.${pkgs.system};
    in
      pkgs.writeShellScriptBin "cargo-expand" ''
        export RUSTC="${nightly'}/bin/rustc";
        export CARGO="${nightly'}/bin/cargo";
        exec "${pkgs.cargo-expand}/bin/cargo-expand" "$@"
      '');

    cargo-public-api' = eachSystem (pkgs: let
      nightly' = nightly.${pkgs.system};
      fakeRustup = pkgs.writeShellScriptBin "rustup" ''shift 3; ${pkgs.lib.getExe' nightly' "cargo"} "$@"'';
    in
      pkgs.writeShellScriptBin "cargo-public-api" ''
        export RUSTC="${nightly'}/bin/rustc";
        export CARGO="${nightly'}/bin/cargo";
        export PATH="${fakeRustup}/bin:${nightly'}/bin:$PATH";
        exec "${pkgs.cargo-public-api}/bin/cargo-public-api" "$@"
      '');

    treefmtEval = eachSystem (pkgs: treefmt-nix.lib.evalModule pkgs ./treefmt.nix);
  in {
    # You can use crane to build the Rust application with Nix

    # packages = eachSystem (pkgs: let
    #   craneLib = inputs.crane.lib.${pkgs.system};
    # in {
    #   default = craneLib.buildPackage {
    #     src = craneLib.cleanCargoSource (craneLib.path ./.);
    #   };
    # });

    devShells = eachSystem (pkgs: {
      # Based on a discussion at https://github.com/oxalica/rust-overlay/issues/129
      default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          clang
          gdb
          # Use mold when we are runnning in Linux
          (lib.optionals stdenv.isLinux mold)
        ];
        buildInputs = [
          rustToolchain.${pkgs.system}
          pkgs.rust-analyzer-unwrapped
          pkgs.cargo
          pkgs.cargo-insta
          pkgs.cargo-hack
          cargo-expand'.${pkgs.system}
          cargo-public-api'.${pkgs.system}
          pkgs.bacon
          # pkgs.pkg-config
          # pkgs.openssl
        ];
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
      };
    });

    formatter = eachSystem (pkgs: treefmtEval.${pkgs.system}.config.build.wrapper);

    checks = eachSystem (pkgs: {
      formatting = treefmtEval.${pkgs.system}.config.build.check self;
    });
  };
}
