{
  lib,
  rustPlatform,
  pkg-config,
}:

rustPlatform.buildRustPackage {
  pname = "board";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let
        base = baseNameOf path;
      in
      # Drop heavy / irrelevant trees from the Nix build context.
      !(
        lib.hasInfix "/dist/" path
        || lib.hasInfix "/.git/" path
        || lib.hasInfix "/node_modules/" path
        || lib.hasInfix "/apps/desktop/dist/" path
        || lib.hasInfix "/apps/desktop/src-tauri/target/" path
        || base == "target"
        || base == "board-windows-x64.exe"
        || base == "board-desktop-linux-x64"
      );
  };

  cargoLock.lockFile = ../Cargo.lock;

  # Workspace root has no [package]; build only the agent CLI.
  cargoBuildFlags = [ "--package" "board" ];
  cargoTestFlags = [ "--package" "board" "--package" "board-core" ];

  nativeBuildInputs = [ pkg-config ];

  meta = {
    description = "Agent-friendly kanban board CLI (shared SQLite store)";
    homepage = "https://github.com/ningen/laneboard";
    license = lib.licenses.mit;
    mainProgram = "board";
    platforms = lib.platforms.unix;
  };
}
