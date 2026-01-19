# fujicli

A CLI to manage Fujifilm devices, simulations, backups, and rendering.

## Status

This tool has only been extensively tested with the **Fujifilm X-T5**, as it is the sole camera I own. While the underlying PTP commands may be compatible with other models, **compatibility is not guaranteed**.

**Use this software at your own risk.** I am not responsible for any damage, loss of data, or other adverse outcomes - physical or psychological - to your camera or equipment resulting from the use of this program.

This project is currently under heavy development. Contributions are welcome. If you own a different Fujifilm camera, testing and reporting compatibility is highly appreciated.

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

It just works, because Linux is simply better. If you do hit permission issues, add a udev rule for Fujifilm devices (vendor ID `0x04cb`).

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

- Send `dump.log` to the maintainers. This helps map PTP properties and behaviors.

- Rendering is especially involved and typically needs extra per-camera reverse engineering; dumps are invaluable but additional iteration is expected.

## Resources

This project builds upon the following fantastic reverse-engineering efforts:

* [fujihack](https://github.com/fujihack/fujihack)
* [fudge](https://github.com/petabyt/fudge)
* [libpict](https://github.com/petabyt/libpict)
* [fp](https://github.com/petabyt/fp)
* [libgphoto2](https://github.com/gphoto/libgphoto2)
