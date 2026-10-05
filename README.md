# fujicli

```
A CLI to manage Fujifilm devices, simulations, backups, and rendering

Usage: fujicli [OPTIONS] <COMMAND>

Commands:
  device      Manage devices
  simulation  Manage film simulations
  backup      Manage backups
  image       Manage and render images
  help        Print this message or the help of the given subcommand(s)

Options:
  -j, --json               Format output using json
  -v, --verbose...         Log extra debugging information (multiple instances increase verbosity)
  -d, --device <DEVICE>    Manually specify target device using USB <BUS>.<ADDRESS>
      --emulate <EMULATE>  Treat device as a different model using <VENDOR_ID>:<PRODUCT_ID>
  -h, --help               Print help
  -V, --version            Print version
```

Only the X-T5 is extensively tested. Other models may work, but
**compatibility is not guaranteed**. Use this software at your own risk.

## Installation

Linux x86_64 binaries are attached to each
[release](https://git.karaolidis.com/karaolidis/fujicli/releases).

With Nix:

```sh
nix run git+https://git.karaolidis.com/karaolidis/fujicli
```

or add the flake's `overlays.default` and install the `fujicli` package, which
includes the manual pages.

From source, you need Rust (edition 2024), [CUE](https://cuelang.org/) on
`PATH`, and the `libusb-1.0` headers:

```sh
cargo build --release
```

`image render --like` and `image extract` also need `exiftool` on `PATH`.

### USB Access

On Linux, if listing devices fails with a permission error, allow access to
Fujifilm's vendor ID:

```udev
# /etc/udev/rules.d/70-fujifilm.rules
SUBSYSTEM=="usb", ATTRS{idVendor}=="04cb", MODE="0666"
```

On Windows, replace the camera's driver with WinUSB using
[Zadig](https://zadig.akeo.ie/). macOS needs no setup.

## Documentation

| Page              | Covers                                                 |
| ----------------- | ------------------------------------------------------ |
| `fujicli(1)`      | Commands and options.                                  |
| `fujicli-fml(5)`  | The Fuji Modelling Language used to describe cameras.  |
| `fujicli(7)`      | Supported cameras, how it works, adding a camera.      |

The pages are in `man/`. Read them without installing with
`man -l man/fujicli.1`.

## Contributing

The canonical repository is on
[Gitea](https://git.karaolidis.com/karaolidis/fujicli), with a
[GitHub mirror](https://github.com/karaolidis/fujicli). See
[CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks and testing.

## Acknowledgements

fujicli builds on reverse-engineering work by
[fujihack](https://github.com/fujihack/fujihack),
[fudge](https://github.com/petabyt/fudge),
[libpict](https://github.com/petabyt/libpict),
[fp](https://github.com/petabyt/fp), and
[libgphoto2](https://github.com/gphoto/libgphoto2).
