# Reverse Engineering Fujifilm Cameras

```
Reverse engineer device communication

Only run this if you have a full device backup and know what you are doing. Misuse can corrupt your camera or void your warranty.

Usage: fujicli device reverse [OPTIONS] <COMMAND>

Commands:
  backup      Attempt to manage backups
  info        Attempt to get camera info
  simulation  Get information about supported simulation management commands
  help        Print this message or the help of the given subcommand(s)

Options:
  -j, --json
          Format output using json

  -v, --verbose...
          Log extra debugging information (multiple instances increase verbosity)

  -d, --device <DEVICE>
          Manually specify target device using USB <BUS>.<ADDRESS>

      --emulate <EMULATE>
          Treat device as a different model using <VENDOR_ID>:<PRODUCT_ID>

  -h, --help
          Print help (see a summary with '-h')
```

## Workflow

- Ensure you have a full device backup.
- Run commands with tracing enabled: `-vvv`.

### Info Support

- Verify support for info commands by running `fujicli device reverse info`.
- Successful output includes a hex dump of the raw PTP response.

### Backup Support

- Verify support for backup commands by running `fujicli device reverse backup export <OUTPUT>`.
- This should generate a full backup file at `<OUTPUT>`. You can them attempt to import it back to the camera using `fujicli device reverse backup import <INPUT>`.

### Simulation Support

- Run `fujicli device reverse simulation` to test support for all known PTP codes and simulation slots.
- Errors are not necessarily fatal; some cameras may only support a subset of commands or slots.
- Allowed values are not verified, we only read from the device.

### Rendering Support

Rendering support is **very complicated** and still largely experimental. Fujifilm does not expose conversion profiles or rendering parameters in any clean or documented way, so this work relies on observing raw USB traffic while official tools manipulate the device.

At a high level, the goal is to map conversion profile byte offsets by watching how Fujifilm X RAW Studio communicates rendering changes to the camera over PTP.

My usual setup looks like this:

- Windows QEMU VM
  - Used exclusively to run Fujifilm X RAW Studio.
  - You can create a minimal installation ISO without dealing with Microsoft's bullshit by using [https://schneegans.de/windows/unattend-generator/](https://schneegans.de/windows/unattend-generator/).

- Wireshark
  - A patch is required to properly expose `frame.raw` in `tshark` for USB traffic, see [https://git.karaolidis.com/karaolidis/nix/src/branch/main/overlays/wireshark/frame-raw.patch](https://git.karaolidis.com/karaolidis/nix/src/branch/main/overlays/wireshark/frame-raw.patch).

1. Load the `usbmon` kernel module.
2. Run `support/monitor-rendering.sh` *before* connecting the camera, see [https://gitlab.com/wireshark/wireshark/-/issues/20908](https://gitlab.com/wireshark/wireshark/-/issues/20908).
3. Start the Windows VM.
4. Connect the camera and pass it through directly to the VM.
5. Open Fujifilm X RAW Studio.
6. Make rendering changes (film simulations, tone curves, etc.). You don't actually need to "finalize" the render, the preview is enough.
7. Observe the USB PTP traffic, conversion profile bytes will be printed as they change.

### EXIF Parsing Support

TBD - can be automated later using bash scripts that render images via `fujicli` and diff `exiftool` output.

### Known Pitfalls

- With at least one camera (X-T5), X RAW Studio is known to receive less conversion profile fields than it sends back. God knows why this happens.
