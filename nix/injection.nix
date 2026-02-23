{ stdenv
, nodejs
, pnpmConfigHook
, fetchPnpmDeps
, pnpm
}:
let
  package = builtins.fromJSON (builtins.readFile ../package.json);

  name = "TidaLuna";
  pname = name;
  src = ./..;
  inherit (package) version;

  # Raw fixed-output derivations for NixOS and Darwin.
  nixOSDepsRaw = fetchPnpmDeps {
    inherit src version name pname;
    fetcherVersion = 1;
    hash = "sha256-Oj34rQbKbsHnqPdVv+ti8z+gZTT+VOsDxg/MQ22sLRQ=";
  };

  darwinDepsRaw = fetchPnpmDeps {
    inherit src version name pname;
    fetcherVersion = 1;
    hash = "sha256-Oj34rQbKbsHnqPdVv+ti8z+gZTT+VOsDxg/MQ22sLRQ=";
  };

  # Wrapper derivations that add pname/version so `nix-update` can
  # treat them as subpackages. The underlying expression (and thus
  # the hash it updates) still lives in this file.
  nixOSDeps = nixOSDepsRaw.overrideAttrs (_: {
    pname = "${pname}-nixos-deps";
    inherit version;
  });

  darwinDeps = darwinDepsRaw.overrideAttrs (_: {
    pname = "${pname}-darwin-deps";
    inherit version;
  });
in
stdenv.mkDerivation rec {
  inherit pname src version name;

  nativeBuildInputs = [
    nodejs
    pnpm
    pnpmConfigHook
  ];

  pnpmDeps =
    if stdenv.hostPlatform.isDarwin
    then darwinDepsRaw
    else nixOSDepsRaw;

  buildPhase = ''
    runHook preBuild
    pnpm install
    pnpm run build
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    cp -R dist $out
    runHook postInstall
  '';

  passthru = {
    inherit nixOSDeps darwinDeps;
  };
}
