- [ ] ADD DOCUMENTATION AND TESTS!!
- [x] rename `basel` -> ~~`they-theme` ?~~ ~~`theythemer`~~ `theymer`
- [x] put all crates flatly into `crates`
  - [x] refactor `tools/cargo-bin` to ~~`crates/xtask`~~ `crates/cargo-bin`
  - [ ] make `themes/cutiepro` cutiepro's canonical _theme source_?
    - [ ] publish to github.com/cutiepro/* as read-only destinations?

- [ ] have meta ascii fields use the same smart object system as swatches, e.g.
      `blurb_ascii` => `blurb.ascii`
- [ ] genericize the provider regex and use percent-encoding (and some sort of
      template system?)
  - [ ] consolidate `output/upstream.rs` and `templates/providers.rs` into
        `upstream.rs`?
- [ ] fix files being written to disk, then reread, formatted and written to
      disk again during rendering
  - [ ] refactor loading to be more generic and atomic across the package
    - [ ] and make it async / multi-threaded?
- [ ] add catppuccin schemes, for testing
- [ ] figure out how schemes/themes should actually be organized
- [ ] see if `itertools` can be used in more places
- [ ] replace `git2` with `gix`?
- [ ] ~~consider if host/provider matching with both `globset` and `regex`
      should be refactored and if it should use a different strategy~~
- [ ] move `render/context.rs` back to `schemes`?
- [ ] investigate `biome` (I think?) formatting suckily
- [ ] make file operations atomic?
- [ ] figure out how to refactor away the `*_internal` functions
- [ ] add checking for invalid directories/templates within the `templates`
      directory when the `render` directory is `.`/root, things that would
      clobber important files (`.theymer`, `templates`)? this might not be
      necessary
- [ ] look into integrating `GitCache` with manifests
- [ ] alias `-p`/`--preview` to `--dry`/`--dry-run`
- [ ] alias `-s`/`--skip` to `-k`/`--keep`
- [ ] reoganize `TODO.md` lol
- [ ] fix `failed to discover git repo from path`
- [ ] add `prune` flag and implement checking for orphaned files
- [ ] figure out strategy for the cli getting/using the default templates
- [ ] add functionality for committing and pushing updates in port subrepos
      automatically?
- [ ] more comprehensive `dry_run` behavior
- [ ] make jinja macro to generate palette table markdown (with a column for the
      roles assigned to each swatch)
- [ ] check for scheme name collisions
- [ ] put extra settings back into the kitty template?
- [ ] add json schema for schemes?
- [ ] use strum in `render::objects::Color`?
- [ ] normalize file extensions
- [ ] organzize items in files (`pub`lic and more general items first)
- [ ] use consistent order for organizing `use` and `mod` statements:
      `use core::{...}` -> `use std::{...}` -> `use ...` -> `use crate::{...}`
      -> `mod` -> `use self::{...}` -> `pub mod ...` / `pub(crate) mod ...` ->
      `pub use::{...}` / `pub(crate) use::{...}`
- [ ] make sure theymer uses the workspace / repo / `theymer.toml` root
- [ ] invalidate manifest cache on config changes?
- [x] `rev` to `ref` and `domain` to `host`, so `Host` should probably go back
      to `Provider` all-around
- [x] optimize `flake.nix`
- [x] fix `#:tombi format.disabled = true` being added twice
- [x] nest theymer modules, there are getting to be too many for a flat
      structure
  - [x] increase encapsulation between (sub)modules
- [x] fix json formatting using trailing commas for vanilla json

# errors

- [ ] ~~consolidate submodule errors into module error type~~ probably not
      actually
- [ ] fix unknown variables in templates returning `InternalBug` (might already
      be fixed?)
  - [ ] have it show the unknown variable
- [ ] make error messages more consistent and less redundant in some cases and
      more meaningful in others
- [ ] collect non-fatal errors and then return instead of exiting immediately
  - [ ] group errors together to lessen unnecessary verbosity
- [ ] print more logs by default
- [ ] if an undefined role is used as a value in a scheme, ask the user if they
      meant to prefix it with a `$` in the error message
- [ ] improve `theymer.config` error handling
- [x] ~~investigate whether it's a good idea that both `Error` and `RenderError`
      convert from `SchemeError` and `TemplateError`~~ refactored to use anyhow
      for these situations
- [x] ~~streamline macro invocations?~~ removed error macros entirely

# templates

- [ ] investigate why `#:tombi lint.disabled = true` isn't being stripped
- [ ] refactor the `set` test to extend the behavior of `defined`, so it can
      consistently be used in templates

# schemes

- [ ] move `scheme` (and the upstream stuff) under `meta`?
  - [ ] probably not actually
    - [ ] maybe remove `scheme` as a key in scheme definitions and just let
          `scheme_ascii` be definable that way so the filename has to be
          authoritative?
- [ ] make `meta`'s ascii versions defined in the same way as swatches, i.e.
      `[meta]
      scheme = "cutiepro"` for defining the display name only and
      accepting automatic ascii fallbacks if necessary, and
      `[meta]
      scheme = { name = "cutiepró", ascii = "cutiedumb" }` for
      explicitly setting the ascii version
- [ ] add option to manually set / override the main upstream repo (project
      root)?
  - [ ] and some way to set / override `upstream_repo` and `upstream_file` in
        templates?
    - [ ] also maybe `upstream_repo` as it's currently defined isn't ever
          actually useful?
- [x] give `scheme` an ascii fallback too. (also check it for length?)
- [x] check `Meta` fields for length

## roles

- [ ] configurable helix picker columns?
- [ ] add `scroll` role (set to `strawberry` for cutiepro?)
- [ ] allow direct hex colors as role values in schemes and make the palette
      table technically optional?
- [x] rethink role design around headings and rainbow punctuation
  - [x] make `rainbow` an optional array under a new ~~`features`~~ `extra`
        table

## palettes

- [ ] add oklch support
  - [ ] replace hex_color with palette
  - [ ] add support for other formats like hsl and hsv?
  - [ ] add option to normalize palette to a single color space / format

# cutiepro

- [ ] change `info` color to something less used?
- [ ] differentiate column headers in Helix from normal foreground text
- [ ] add four additional colors to the rainbow brackets for a total of 10
      levels
- [x] make `creamsicle` slightly yellower (or `sand` slightly redder?)

# personal

- [ ] show current directory and running command in fish title
- [x] fix my custom Helix move lines bindings not working right with multi-line
      selections
