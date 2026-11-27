let
  pkgs = import (builtins.fetchTarball {
    url = "https://github.com/nixos/nixpkgs/archive/b4fd65b198c599cbe814fcb9f42d25d021595ec9.tar.gz";
    sha256 = "1a241q5kynd8zg3s169n5pklkpxzsqpqiwy83izg2jlbalvsnmwa";
  }) { };
  fenix = import (builtins.fetchTarball {
    url = "https://github.com/nix-community/fenix/archive/9aadb1d479ad081f67eac50920558e140f26d167.tar.gz";
    sha256 = "08dv0kybhc5hrc0mafy381fj8zwd8jm8zi1yjwc0jn6xdvsk18rm";
  }) { inherit pkgs; };
  toolchain = fenix.fromToolchainFile { file = ./rust-toolchain.toml; };
  # Shared libraries winit, wgpu, and audio open at runtime on NixOS.
  runtimeLibs = with pkgs; [
    vulkan-loader
    libxkbcommon
    wayland
    libGL
    libx11
    libxcursor
    libxi
    libxrandr
    alsa-lib
  ];
  # Mesa's software Vulkan driver (llvmpipe), offered only when the environment asks
  # for it with SUPERCOM_SOFTWARE_VULKAN=1: CI runners have no graphics adapter. Without
  # the request the shell sets no driver variable, so a developer's loader finds the
  # devices it always did. The request makes the software driver the only one the loader
  # is given, so a test run names it and nothing else.
  softwareVulkan = builtins.getEnv "SUPERCOM_SOFTWARE_VULKAN" == "1";
  softwareDriver = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.${pkgs.stdenv.hostPlatform.parsed.cpu.name}.json";
in
pkgs.mkShell (
  {
    packages = with pkgs; [
      toolchain
      cargo-component
      cargo-deny
      cargo-nextest
      wasm-tools
      wasmtime
      nickel
      just
      treefmt
      nixfmt
      prettier
      shfmt
      taplo
      pkg-config
      python3
      unzip
      vulkan-tools
      vulkan-validation-layers
      renderdoc
    ];
    RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
    VK_LAYER_PATH = "${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d";
  }
  // pkgs.lib.optionalAttrs softwareVulkan { VK_DRIVER_FILES = softwareDriver; }
)
