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
  -j, --json                   Format output using json
  -v, --verbose...             Log extra debugging information (multiple instances increase verbosity)
  -d, --device <DEVICE>        Manually specify target device using the ID reported by `device list`
      --transport <TRANSPORT>  Transport used to reach the camera [default: auto] [possible values: auto, wpd, libusb]
      --emulate <EMULATE>      Treat device as a different model using <VENDOR_ID>:<PRODUCT_ID>
  -h, --help                   Print help
  -V, --version                Print version
```

## Status

Extensively tested only with the **Fujifilm X-T5**. The underlying PTP commands
likely work on other Fujifilm models, but **compatibility is not guaranteed**.

**Use this software at your own risk.** The author is not responsible for any
damage, lost data, or other adverse outcomes - physical or psychological - to
your camera or equipment.

This project is under heavy development. Contributions are welcome. See
[docs/users/support.md](docs/users/support.md) for the camera support matrix.

## Documentation

The full wiki lives in [`docs/`](docs/README.md).

## GitHub Mirror

The canonical source for `fujicli` lives on a
[self-hosted Gitea instance](https://git.karaolidis.com/karaolidis/fujicli). A
[GitHub mirror](https://github.com/karaolidis/fujicli) exists for visibility and
community collaboration:

- Stars, issues, and pull requests on GitHub are welcome.
- Changes may be reviewed and merged on the primary Gitea repo first.
- GitHub may lag slightly behind the canonical repo during heavy development.

If you're looking for the absolute latest commits, the self-hosted repo is the
source of truth.

## Resources

This project builds upon the following reverse-engineering efforts:

- [fujihack](https://github.com/fujihack/fujihack)
- [fudge](https://github.com/petabyt/fudge)
- [libpict](https://github.com/petabyt/libpict)
- [fp](https://github.com/petabyt/fp)
- [libgphoto2](https://github.com/gphoto/libgphoto2)
