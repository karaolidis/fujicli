# Usage

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

Every subcommand has a short alias: `device -> d`, `simulation -> s`,
`backup -> b`, `image -> i`. Within a subcommand, common operations are also
aliased (`list -> l`, `get -> g`, `set -> s`, `export -> e`, `import -> i`,
`render -> r`).

The `-d / --device` flag takes the opaque device ID printed by
`fujicli device list` and is only needed when more than one supported camera is
plugged in. Its format depends on the transport: a USB bus/address pair (e.g.
`1.4`) for libusb, a Windows Portable Devices device ID for WPD. An unambiguous
substring of the ID (e.g. the serial number) is also accepted.

`--transport` selects how the camera is reached. `auto` (the default) tries WPD
first on Windows and falls back to libusb, so both stock-driver and
Zadig/WinUSB setups work; `wpd` and `libusb` force one of them.

`--emulate VENDOR:PRODUCT` forces fujicli to treat the connected device as a
different model - useful for development; see
[camera support](support.md#emulation-mode).

## Devices

```sh
# List connected supported cameras.
fujicli device list

# Print extended info for the currently selected camera (model, serial,
# battery, USB mode).
fujicli device info
```

## Backups

Backups are camera-native blobs; treat them as opaque.

```sh
fujicli backup export camera.fbk  # write to file
fujicli backup export -           # write to stdout
fujicli backup import camera.fbk
```

## Simulations

A _simulation_ is one of the camera's custom-setting slots (e.g. C1-C7). The
number of slots is per-camera (`SLOTS` in the generated code).

```sh
# List slots with their assigned names.
fujicli simulation list

# Read one slot.
fujicli simulation get c1

# Update fields on a slot. Any subset is allowed; the rest is read from
# the camera and the result validated.
fujicli simulation set c1 \
  --film-simulation reala-ace \
  --grain-effect weak-small \
  --white-balance auto

# Round-trip JSON to disk.
fujicli simulation export c1 c1.json
fujicli simulation import c1 c1.json
```

The exact set of `--<field>` flags is generated from the FML schema; run
`fujicli simulation set --help` to list what your build supports. Aliases work -
both `--white-balance auto` and `--white-balance Auto` parse to the same
variant, and most options accept short forms (e.g. `mono` for `monochrome`).
Pass `--json` for machine-readable output on `get`/`list`.

## Images

```sh
# Render a RAF in-camera using the active settings.
fujicli image render input.raf out.jpg

# Render using slot C1's settings.
fujicli image render --slot c1 input.raf out.jpg

# Render using a previously-exported simulation.
fujicli image render --simulation-file c1.json input.raf out.jpg

# Override individual fields on top of any of the above.
fujicli image render --slot c1 \
  --film-simulation classic-chrome \
  --grain-effect off \
  input.raf out.jpg

# Faster but lower quality preview render.
fujicli image render --draft input.raf out.jpg
```

The render command always layers in this order: simulation source (slot or
file), then any inline `--<field>` overrides. Fields your CLI flags don't set
are pulled from the camera's current state.

Use `-` in place of any input or output filename to read from stdin or write to
stdout.

## Output and Logging

`-j / --json` switches list/get commands to pretty JSON. Without it, output is
human-readable.

`-v` (repeatable: `-v`, `-vv`, `-vvv`) raises log verbosity. At `-vvv` you get
full PTP byte-dumps, which is what you want when filing a bug.
