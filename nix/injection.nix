{
  stdenv,
  nodejs,

  pnpmConfigHook,
  fetchPnpmDeps,
  pnpm,
}:
let
  package = builtins.fromJSON (builtins.readFile ../package.json);

  name = "TidaLuna";
  pname = "${name}";
  src = ./..;
  inherit (package) version;

  nixOSDeps = fetchPnpmDeps {
    inherit pname src version;
    fetcherVersion = 1;
    hash = "sha256-Oj34rQbKbsHnqPdVv+ti8z+gZTT+VOsDxg/MQ22sLRQ=";
  };

  darwinDeps = fetchPnpmDeps {
    inherit pname src version;
    fetcherVersion = 1;
    hash = "sha256-pHIY4Ie66ZVwEne/4RmY2QvsRWcnfsl2kv3CDXcqVrg=";
  };

in
stdenv.mkDerivation (rec {

  inherit
    pname
    src
    version
    name
    ;

  nativeBuildInputs = [
    nodejs

    pnpm
    pnpmConfigHook
  ];

  pnpmDeps = if stdenv.isDarwin then darwinDeps else nixOSDeps;

  buildPhase = ''
    runHook preBuild

    pnpm install
    pnpm run build

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    cp -R "dist" "$out"

    runHook postInstall
  '';

})
