# fujicli

```
A CLI to manage Fujifilm devices, simulations, backups, and rendering

Usage: fujicli [OPTIONS] <COMMAND>

Commands:
  device      Manage devices
  simulation  Manage film simulations
  backup      Manage backups
  render      Render images using in-camera processor
  help        Print this message or the help of the given subcommand(s)

Options:
  -j, --json               Format output using json
  -v, --verbose...         Log extra debugging information (multiple instances increase verbosity)
  -d, --device <DEVICE>    Manually specify target device using USB <BUS>.<ADDRESS>
      --emulate <EMULATE>  Treat device as a different model using <VENDOR_ID>:<PRODUCT_ID>
  -h, --help               Print help
  -V, --version            Print version
```

## Status

This tool has only been extensively tested with the **Fujifilm X-T5**, as it is the sole camera I own. While the underlying PTP commands may be compatible with other models, **compatibility is not guaranteed**.

**Use this software at your own risk.** I am not responsible for any damage, loss of data, or other adverse outcomes -- physical or psychological -- to your camera or equipment resulting from the use of this program.

This project is currently under heavy development. Contributions are welcome. If you own a different Fujifilm camera, testing and reporting compatibility is highly appreciated.

## GitHub Mirror

The canonical source for `fujicli` lives on my [self-hosted Gitea instance](https://git.karaolidis.com/karaolidis/fujicli). A [GitHub mirror](https://github.com/karaolidis/fujicli) exists purely for visibility and community collaboration. In practice, this means:

- Stars, issues, and  pull requests on GitHub are welcome
- Changes may be reviewed and merged on the primary Gitea repo first
- GitHub may lag slightly behind the canonical repo during heavy development

If you're contributing, testing new camera models, or reporting bugs, GitHub is totally fine. If you're looking for the absolute latest commits, the self-hosted repo is the source of truth.

## OS Notes

### Windows

To allow `fujicli` to talk to the camera, install the `WinUSB` driver for your Fujifilm device using Zadig.

- Install Zadig: https://zadig.akeo.ie/
- Connect your camera via USB in PTP/USB mode.
- Open Zadig -> Options -> "List All Devices".
- Select your camera (it may appear as "USB PTP" or with the model name).
- Choose `WinUSB` (recommended) or `libusbK` as the target driver.
- Click "Replace Driver". This disables Windows' default WPD/photo import. You can revert later from Zadig.

### MacOS

Usually no driver changes are required.

### Linux

It just works, because Linux is simply better ;).

If you do hit permission issues, add a udev rule for Fujifilm devices (vendor ID `0x04cb`).

## Camera Support

The following cameras are currently recognized. Feature support varies per model/generation:

| Model           | Generation  | Base Info | Backups | Simulations | Rendering |
| --------------- | ----------- | --------- | ------- | ----------- | --------- |
| FUJIFILM X-E1   | X-Trans     | ?         |         |             |           |
| FUJIFILM X-M1   | X-Trans     | ?         |         |             |           |
| FUJIFILM X70    | X-Trans II  | ?         |         |             |           |
| FUJIFILM X-E2   | X-Trans II  | ?         |         |             |           |
| FUJIFILM X-T1   | X-Trans II  | ?         |         |             |           |
| FUJIFILM X-T10  | X-Trans II  | ?         |         |             |           |
| FUJIFILM X100F  | X-Trans III | ?         |         |             |           |
| FUJIFILM X-E3   | X-Trans III | ?         |         |             |           |
| FUJIFILM X-H1   | X-Trans III | ?         |         |             |           |
| FUJIFILM X-Pro2 | X-Trans III | ?         |         |             |           |
| FUJIFILM X-T2   | X-Trans III | ?         |         |             |           |
| FUJIFILM X-T20  | X-Trans III | ?         |         |             |           |
| FUJIFILM X100V  | X-Trans IV  | ?         |         |             |           |
| FUJIFILM X-E4   | X-Trans IV  | ?         |         |             |           |
| FUJIFILM X-Pro3 | X-Trans IV  | ?         |         |             |           |
| FUJIFILM X-S10  | X-Trans IV  | ?         |         |             |           |
| FUJIFILM X-T3   | X-Trans IV  | ?         |         |             |           |
| FUJIFILM X-T4   | X-Trans IV  | ?         |         |             |           |
| FUJIFILM X-S20  | X-Trans IV  | ✓         | ✓       | ✓           |           |
| FUJIFILM X100VI | X-Trans V   | ?         |         |             |           |
| FUJIFILM X-H2   | X-Trans V   | ?         |         |             |           |
| FUJIFILM X-H2S  | X-Trans V   | ?         |         |             |           |
| FUJIFILM XT-5   | X-Trans V   | ✓         | ✓       | ✓           | ✓         |

### Legend

- **✓**: Known to work
- **?**: Untested but likely works
- **✗**: Known not to work
- Blank: Unimplemented until further information is available

## Help Add Your Camera

If your camera isn't listed, or a feature is missing, you can help expedite support:

- Run a detailed device dump and share the log:

	`fujicli device dump -vvv > dump.log 2>&1`

- Open an issue, or send `dump.log` to the maintainers. This helps map PTP properties and behaviors.

- Rendering is especially involved and typically needs extra per-camera reverse engineering; dumps are invaluable but additional iteration is expected.

### Emulation Mode (`--emulate`)

The `--emulate` flag forces `fujicli` to treat the connected camera as a different Fujifilm model by overriding its USB vendor/product ID. This is primarily intended for development, reverse-engineering, and compatibility testing.

- Emulation does not magically add support for unsupported cameras
- It may expose incorrect or unsupported PTP properties
- Some commands (especially rendering) can and will behave unpredictably

If you're not actively debugging or contributing, you probably don't want this flag.

## Resources

This project builds upon the following fantastic reverse-engineering efforts:

* [fujihack](https://github.com/fujihack/fujihack)
* [fudge](https://github.com/petabyt/fudge)
* [libpict](https://github.com/petabyt/libpict)
* [fp](https://github.com/petabyt/fp)
* [libgphoto2](https://github.com/gphoto/libgphoto2)
