# Installation

Linux x86_64 binaries are published by the Gitea CI on every tag. See the
[releases page](https://git.karaolidis.com/karaolidis/fujicli/releases). For
other platforms, or for the latest commits, build from source.

## NixOS / Nix

```sh
nix run git+https://git.karaolidis.com/karaolidis/fujicli
```

Or add the flake to your system inputs and use the `fujicli` package from
`overlays.default`.

For a dev shell with the right toolchain (`cue`, `cargo`, fenix nightly):

```sh
git clone https://git.karaolidis.com/karaolidis/fujicli
cd fujicli
nix develop
cargo build --release
```

## From Source (non-Nix)

You need:

- A recent Rust toolchain (edition 2024). Install via
  [rustup](https://rustup.rs/).
- [CUE](https://cuelang.org/) on `PATH` - the build script invokes `cue export`
  to materialize the schema into JSON.
- A C toolchain and `libusb-1.0` headers, for the `rusb` dependency.

Then:

```sh
git clone https://git.karaolidis.com/karaolidis/fujicli
cd fujicli
cargo build --release
./target/release/fujicli --help
```

## Per-Platform Notes

### Linux

Usually no extra setup. If you hit permission errors when listing devices, add a
`udev` rule for Fujifilm's vendor ID (`0x04cb`):

```udev
# /etc/udev/rules.d/70-fujifilm.rules
SUBSYSTEM=="usb", ATTRS{idVendor}=="04cb", MODE="0666"
```

Reload with `sudo udevadm control --reload-rules && sudo udevadm trigger`.

### macOS

Usually no driver changes are required. Connect the camera, make sure it is in
PTP / USB mode in its menus, and `fujicli device list` should see it.

### Windows

Windows binds the camera to its default WPD / photo-import driver, which blocks
raw PTP. Replace the driver with WinUSB or libusbK using Zadig:

1. Install Zadig from <https://zadig.akeo.ie/>.
2. Connect the camera in PTP / USB mode.
3. In Zadig: **Options -> List All Devices**.
4. Select the camera (often listed as "USB PTP" or by model name).
5. Pick **WinUSB** (recommended) or **libusbK** as the target driver.
6. Click **Replace Driver**. You can revert from Zadig later.
