# Runtime

The runtime is the small Rust crate that mounts the `src/lib/generated/` module
and dispatches user requests through trait objects. There is deliberately not
much here; the heavy lifting is at build time.

## The Trait Hierarchy

`CameraBase` is the root. Every camera struct implements it; feature
participation is signalled by overriding `as_<feature>_*` methods that default
to `None`.

```rust
pub trait CameraBase {
    type Context: rusb::UsbContext;

    fn camera_definition(&self) -> &'static SupportedCamera;
    fn chunk_size(&self) -> usize { 1024 * 1024 }

    fn as_backup_manager(&self) -> Option<&dyn CameraBackupManager<...>> { None }
    fn as_simulation_parser(&self) -> Option<&dyn CameraSimulationParser> { None }
    fn as_simulation_manager(&self) -> Option<&dyn CameraSimulationManager<...>> { None }
    fn as_render_manager(&self) -> Option<&dyn CameraRenderManager<...>> { None }

    fn get_info(&self, ptp: &mut Ptp) -> anyhow::Result<Box<dyn CameraInfo>> { ... }
}
```

The codegen emits overrides only for the features the camera spec declares:

```rust
// src/lib/generated/cameras.rs (sketch)
impl CameraBase for XT5 {
    type Context = rusb::GlobalContext;
    fn camera_definition(&self) -> &'static SupportedCamera { &C_X_T5 }
    fn chunk_size(&self) -> usize { 16128 * 1024 }

    fn as_backup_manager(&self) -> Option<&dyn CameraBackupManager<...>> { Some(self) }
    fn as_simulation_parser(&self)  -> Option<&dyn CameraSimulationParser>  { Some(self) }
    fn as_simulation_manager(&self) -> Option<&dyn CameraSimulationManager<...>> { Some(self) }
    fn as_render_manager(&self)     -> Option<&dyn CameraRenderManager<...>>     { Some(self) }
}
```

If a camera doesn't declare a feature, the override is omitted, the default
`None` is returned, and the runtime's high-level method errors with a friendly
message.

## Top-Level Dispatch

[`src/lib/mod.rs`](../../src/lib/mod.rs) wraps the trait dispatch:

```rust
impl Camera {
    pub fn export_backup(&mut self) -> anyhow::Result<Vec<u8>> {
        if let Some(backups) = self.r#impl.as_backup_manager() {
            backups.export_backup(&mut self.ptp)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_BACKUP_MANAGEMENT);
        }
    }
    // similar for simulations, render, ...
}
```

The CLI doesn't see the per-camera types at all; it always goes through
`Camera`. Adding a new camera to the schema automatically extends the dispatch
surface; nothing in `src/cli/` needs to know.

## Feature Traits

### `CameraBackupManager`

[`features/backup/manager.rs`](../../src/lib/features/backup/manager.rs)
provides default implementations of `export_backup` and `import_backup` against
`Ptp`; the underlying PTP exchange has been uniform across the cameras we've
seen, so this is a blanket
`impl<T> CameraBackupManager for T where T: CameraBase`. A camera opts in by
overriding `as_backup_manager` to return `Some(self)`.

### `CameraSimulationParser` + `Simulation`

[`features/simulation/`](../../src/lib/features/simulation/).
`CameraSimulationParser` is JSON serialize/deserialize (so users can round-trip
a slot to disk). `Simulation` is the trait object the parser yields: `try_pull`,
`try_push`, `try_update_from`, `name`, `to_base`. Codegen implements both for
every simulation-capable camera.

`SimulationListItem` is a small helper struct used by `simulation
list`.

### `CameraSimulationManager`

The PTP-talking interface: `custom_settings_slots`, `get_simulation`,
`update_simulation`, `set_simulation`. The codegen-emitted impl uses the
option's `SimulationSetting::try_push` to select the slot before
reading/writing.

### `CameraRenderManager`

[`features/render/manager.rs`](../../src/lib/features/render/manager.rs)
provides default `send_image` / `render_image` (uniform across cameras). Each
render-capable camera must override `render`, which the codegen does: send the
image, fetch the current `<Camera>RenderProfile`, `try_update_from(partial)`,
write back, then `render_image`.

For the cross-state semantics that `try_update_from` enables, see
[fml/rules / cross-state rules](../fml/rules.md#cross-state-rules).

## PTP and Option Traits

[`src/lib/ptp/option.rs`](../../src/lib/ptp/option.rs) declares the two
option-level traits the codegen targets:

- **`SimulationSetting`**: for options with a `prop_code`. Carries
  `prop_code() -> u16` plus `try_push` / `try_pull` defaulting to
  `Ptp::set_prop` / `get_prop`.
- **`ConversionProfileField`**: for every option (including inline-only ones).
  Reads/writes the 32-bit lifted form used inside render profiles.

The PTP wire codec lives in [`crates/ptp/cursor`](../../crates/ptp/cursor/)
(`PtpSerialize`, `PtpDeserialize`, `Read`, `ExactString`). The derive macros for
the basic shapes live in [`crates/ptp/macro`](../../crates/ptp/macro/).

## Input Helpers

[`src/lib/input/`](../../src/lib/input/) provides:

- **`CleanAlphanumeric`**: strips whitespace/punctuation and lower-cases a
  string. Used at the start of every option's `FromStr` so variations all
  collide into the same key.
- **`Choices`**: Levenshtein-based closest-match suggestions used in `FromStr`
  error messages.

## Adding a Runtime Feature

If you want `fujicli` to support a brand-new feature (not just adding a camera
to an existing one):

1. Add the data model to `fml/camera.cue` under `#Features`.
2. Add the AST in `crates/codegen/src/ast/cameras.rs` (mirror the structure of
   `Simulation` / `Render`).
3. Add a feature trait in `src/lib/features/<name>/`, with sensible default
   impls if possible, like `CameraBackupManager`.
4. Add `as_<name>(&self)` to `CameraBase`.
5. Add an emitter in `crates/codegen/src/common/<name>/`.
6. Wire dispatch through `Camera` in `src/lib/mod.rs`.
7. If the feature has CLI surface, generate `<Feature>Args` in
   `crates/codegen/src/cli/<name>.rs` and consume it from the appropriate
   `src/cli/` module.

The grammar / DNF / presence / repair machinery is feature-agnostic and can be
reused if your feature has its own typed setting list and validation rules.
