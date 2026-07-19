# XPad3S Android 12 5.10 GKI late-load

This branch adds a physically verified modern-kernel path while preserving the
existing XPad2 Linux 4.19 implementation. XPad3S uses the standard KernelSU
GKI code path; it does not enable `CONFIG_KSU_LEGACY_4_19`.

## Exact verified target

- Product/device: `TALIH_PD3S` / `TALIH-PD3S`
- Android: 13 (`TP1A.220624.014`)
- Fingerprint:
  `alps/TALIH-PD3S/TALIH-PD3S:13/TP1A.220624.014/338:user/release-keys`
- Kernel: `5.10.198-android12-9-00019-g6efebf1322d6-ab11471183`
- Architecture: arm64
- ksud KMI selector: `android12-5.10`

The KMI family is a necessary compatibility boundary, not a substitute for
device validation. The module's undefined symbols were compared with the
exact extracted XPad3S kallsyms and had zero missing symbols before loading.

## Physical-device result

The following state was verified in one clean boot on 2026-07-18:

- `kernelsu` loaded dynamically and remained `Live` in `/proc/modules`;
- KernelSU version `32551`, UAPI version `2`;
- `lkm: true`, `late_load: true`, runtime mode `late-load`;
- KernelSU `su` returned UID/GID 0 in `u:r:ksu:s0`;
- the official `me.weishu.kernelsu` Manager v3.2.5 was signature-accepted,
  pinned as appId `10191`, and obtained the KernelSU driver fd/root;
- KernelSU applied its policy and restored global SELinux to Enforcing.

The late-loaded state is intentionally non-persistent. Reboot returns the
device to the stock kernel state. Do not unload or replace a live module in the
same boot; reboot before testing another build.

## Kernel module build

The validated module was built with the same DDK family and command used by
the upstream workflow:

```sh
docker run --rm --privileged \
  -v "$PWD:/github/workspace" \
  -w /github/workspace/kernel \
  ghcr.io/ylarod/ddk-min:android12-5.10-20260313 \
  sh -lc 'git config --global --add safe.directory /github/workspace && \
    CONFIG_KSU=m CC=clang make && \
    llvm-strip -d kernelsu.ko'
```

Copy the result to the embedded KMI asset before building ksud:

```sh
install -m 0644 kernel/kernelsu.ko \
  userspace/ksud/bin/aarch64/android12-5.10_kernelsu.ko
```

The two modern-kernel source repairs on this branch restore the UAPI mount
flags include required by the DDK and pass the current argument expected by
`reset_avc_cache()`.

## ksud build and checks

```sh
cargo ndk -t arm64-v8a check -p ksud
cargo ndk -t arm64-v8a clippy -p ksud -- -D warnings
cargo fmt --all -- --check

KSU_VERSION_NAME=0.2.1-xpad3s-gki \
  cargo ndk -t arm64-v8a build --release -p ksud
```

This `ksud` embeds both the new `android12-5.10` module and the unchanged
`xpad2-4.19.191` legacy module. Selecting a KMI is explicit at runtime.

The xpad3 control plane may also pass the hidden `--trace-file` diagnostic
argument. It is disabled by default. When enabled, ksud only appends to a
pre-created, shell-owned 0600 regular file below the xpad3 transaction log
root. Every stage is synced independently, including immediately before and
after `init_module`, so a reboot during module initialization leaves a durable
last-stage boundary for the next boot's log export.

## Device use

The loader requires an already authorized temporary root and a writable module
loading path. The verified XPad3S exploit stage also supplied the temporary
permissive SELinux window needed during initialization.

```sh
adb push artifacts/ksud-xpad3s /data/local/tmp/ksud-xpad3s
adb shell chmod 755 /data/local/tmp/ksud-xpad3s

# Run through the already-authorized temporary-root channel.
/data/local/tmp/ksud-xpad3s late-load \
  --kmi android12-5.10 --allow-shell

/data/local/tmp/ksud-xpad3s debug info
printf 'id\nexit\n' | /data/local/tmp/ksud-xpad3s debug su
```

Expected core fields:

```text
version: 32551
uapi_version: 2
lkm: true
late_load: true
runtime_mode: late-load
```

## Artifacts

```text
5e64a90c35b44b8ee3268604020eb31c015ed8b6fb36770e8f07db9ef9a1db7d  artifacts/kernelsu-xpad3s-android12-5.10.ko
7075d06a731c4b0fd2a6c73a7ae0710f2824db0b6db4eff017f39f5fe32a0001  artifacts/ksud-xpad3s
```

The Manager APK is not bundled. Install the official Manager separately.
