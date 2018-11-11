# Run `nix-shell` to be able
# to build Grin on NixOS.
{ pkgs ? import <nixpkgs> {} }:

pkgs.stdenv.mkDerivation {
  name = "rdedup";

  buildInputs = with pkgs; [
    postgresql
  ];

  shellHook = ''
      LD_LIBRARY_PATH=${pkgs.postgresql}/lib/:$LD_LIBRARY_PATH
  '';
}
