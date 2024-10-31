{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs; [
    pkg-config
    openssl
    openssl.dev
    rustc
    cargo
  ];
}
