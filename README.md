# kbdi

A helper tool for manipulating keyboards and locales on Windows. In general, this has no use case outside of keyboard installers.

## Building

Rust 1.98.1 is pinned in `rust-toolchain.toml`; this crate uses edition 2024.
The historical installer payload is i686. The modern helper also builds for
x86_64 and native ARM64 (`aarch64-pc-windows-msvc`). `.cargo/config.toml` enables
static CRT linking on all three targets. The embedded application manifest omits
the optional `processorArchitecture` attribute so it does not declare the wrong
architecture when cross-compiling.

Install Visual Studio's MSVC build tools and Windows SDK, including the ARM64
C++ build tools for ARM64 builds. Rustup installs the pinned Rust target libraries.
An x64 Windows host with those tools can cross-compile the ARM64 executable.

```
cargo build --locked --release --target i686-pc-windows-msvc --bin kbdi
cargo build --locked --release --target x86_64-pc-windows-msvc --bin kbdi
cargo build --locked --release --target aarch64-pc-windows-msvc --bin kbdi
cargo build --locked --release --target i686-pc-windows-msvc --features legacy --bin kbdi-legacy
cargo test --locked --all-targets --target i686-pc-windows-msvc
cargo test --locked --lib --bin kbdi-legacy --features legacy --target i686-pc-windows-msvc
cargo fmt --all -- --check
```

The native ARM64 payload is `target/aarch64-pc-windows-msvc/release/kbdi.exe`;
its matching symbols are `kbdi.pdb` in the same directory. On an x64 host, compile
the ARM64 tests without trying to execute them:

```
cargo test --locked --release --target aarch64-pc-windows-msvc --lib --bin kbdi --no-run
```

Run that command without `--no-run` on ARM64 Windows, then exercise the actual
keyboard installation, activation and removal there. Cross-compilation and PE
inspection do not validate the dynamically loaded Windows input APIs at runtime.
The legacy helper's runtime coverage remains limited to x86/x64.

Clap replaces StructOpt while preserving installer command names and flags.
Microsoft bindings replace direct winapi and the custom HSTRING implementation.
Private dynamic language/input APIs remain isolated in `platform/sys.rs`, use
system-library search rules, and report missing capabilities. Native tests use
read-only queries only; they do not establish keyboard installation compatibility
on old Windows releases. Microsoft windows-registry 0.100.0 replaces the old
registry/utfx wrapper; owned SDK snapshots propagate enumeration errors.
Nineteen modern tests and six legacy-configuration tests pass on each of x86 and x64.
Both complete lockfiles pass cargo-audit 0.22.2 with warnings denied.

Logging is local; compile-time `SENTRY_DSN` no longer enables telemetry.
`--default-user` now reports unsupported without changing the current user's
keyboard, instead of silently ignoring the flag. Default-profile support remains
unfinished.


Keyboard lifecycle fixes preserve unrelated input methods: activation adds only
its requested keyboard, uninstall disables only its product's keyboard before
removing the registration, and cleanup disables only missing custom keyboard
IDs. Ordinary keyboards, existing custom layouts and TSF input methods are not
cleared and rebuilt. Uninstall checks machine deletion permissions before
changing the current user's inputs, and repeated uninstall is a no-op.

Unsupported language tags are rejected before installation writes, activation
reports a missing allocated language ID, and failed activation attempts to
restore the original language list when it added a profile. Windows Boolean
activation failures are propagated to the CLI. Read-only keyboard enumeration
no longer requests machine registry write access. Input-ID parsing rejects
malformed/trailing data without panicking and accepts its serialized hex form.

These fixes have regression coverage for input selection, no-op/error handling,
Boolean API results, parsing, and read-only locale validation. Full installed
keyboard typing, upgrade/uninstall acceptance and default-profile provisioning
remain separate validation work. `registry_regen` remains an explicit repair
command; ordinary activation and uninstall no longer invoke it.

## Live keyboard activation

`keyboard_enable` and `keyboard_install -e` now verify that Windows saved the
requested input method and that the live profiles used by Explorer contain its
correct language/layout ID. On Windows 11, ctfmon can retain a special-layout
cache from before installation and publish `LANG:00000000`; selecting that input
can make Explorer's switcher fail fast. Registry/DLL checks alone miss this.

The CLI retries profile delivery, then restarts ctfmon at most once if the live
snapshot is stale. It verifies the corrected profiles before returning success.
A healthy session does not restart. Recovery checks the caller's user, session,
desktop, shell, and the opened ctfmon process's path/token. It never restarts
Explorer or system services. Windows normally respawns ctfmon; a bounded fallback
launch uses the same user's normal desktop and a non-elevated token. Saved input
preferences remain intact, and the foreground keyboard selection is restored.

```
kbdi keyboard_refresh --check  # verify only; nonzero if stale/unavailable
kbdi keyboard_refresh          # verify and recover if needed
```

For bundles, register every layout first with `keyboard_install` (without `-e`),
then enable them. The first needed recovery rebuilds the cache for all registered
layouts, so later enables can verify without another restart. The outto CI
builder uses this order with the existing install/enable command interface.
Alternatively, `keyboard_install -e --defer-refresh` or
`keyboard_enable --defer-refresh` saves each preference and explicitly defers
verification; finish the entire batch with one `keyboard_refresh`. Deferred
success means the preference was saved, not that live activation was verified.

Run activation/recovery as the intended logged-in user on the normal desktop.
SYSTEM, session zero, alternate desktops, or a different user's shell cannot
refresh another user's session. A nonzero exit explains when interactive recovery
or signing out and back in is required. Installers must handle that result;
outto's current generic run-hook handler only logs nonzero exits as warnings.

The live check uses the private WinRT CoreKeyboardInputProfileManager contract,
isolated in `platform/core_profiles.rs`. A fresh, read-only child process pumps
the asynchronous initial snapshot and is limited to eight seconds. Missing APIs,
invalid data, crashes, and timeouts fail explicitly and do not trigger a restart.
No debugger attachment, memory offsets, DLL injection, or registry reconstruction
is involved. The library's `keyboard::enable` saves/verifies preferences; the CLI
adds live verification (`text_services::refresh` requires the kbdi executable).

Validation on Windows 11 (Core TextInput/InputService 10.0.26100.9278): two new
temporary registrations produced two zero layout IDs; one x64 elevated recovery
corrected both and a repeat was a no-op. A fresh x86 non-elevated `keyboard_enable`
also recovered automatically. Test registrations were removed; the language/input
list, default input override and Explorer process were preserved. Release unit
tests cover healthy/delayed/stale batches, check-only operation, unavailable
probes, and failed recovery. x86 and x64 execute locally; ARM64 is cross-compiled
with test executables and still requires native runtime validation. Older Windows
versions and the explicit launch fallback require additional runtime coverage.

## License

`kbdi` is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
