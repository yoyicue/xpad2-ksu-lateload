# XPad2 KernelSU late-load port

An experimental KernelSU late-load port for the exact XPad2/TALIH PD2
firmware described below. It loads KernelSU at runtime without replacing the
boot image.

To obtain the temporary root required by the loader on the verified firmware,
see [yoyicue/xpad2-ionstack-poc](https://github.com/yoyicue/xpad2-ionstack-poc).
That project implements the pure-C, host-assisted re-root stage; this project
starts at the resulting authorized temporary-root boundary and installs the
runtime KernelSU module.

This repository is based on upstream
[KernelSU](https://github.com/tiann/KernelSU) commit
`ccfee6dc67d79a1404eb71c67b67b4f6c7b65a4a`. The port keeps the upstream GPL
license and adds a guarded Linux 4.19 compatibility path.

## Verified target

- Product/device: `TALIH_PD2` / `ls12_mt8797_wifi_64`
- SoC/kernel family: MediaTek MT8797, arm64
- Android: 13
- Kernel release: `4.19.191+`
- Kernel vermagic: `4.19.191+ SMP preempt mod_unload modversions aarch64`
- OEM compiler: Android clang `r383902`
- ksud KMI selector: `xpad2-4.19.191`

This is an ABI-specific port. A matching model name alone is not sufficient:
kernel build, firmware, symbol visibility and module ABI must match. Do not
load the included module on unrelated devices or firmware revisions.

## Status

Verified on physical hardware:

- late-load from an initially unloaded boot;
- KernelSU version 32547 and UAPI 2;
- KernelSU-provided root shell;
- clean direct `rmmod kernelsu` without reboot;
- clean `ksud unload` while adbd and zygote remain running;
- late-load again in the same boot after unload.

The final kernel module imports 93 runtime symbols, with zero missing from the
verified target's runtime kallsyms.

## Repository layout

- repository root: upstream KernelSU Git history plus the XPad2 port changes.
- `artifacts/`: the exact binaries used for final device validation.
- `PROGRESS.md`: development history, root causes and validation evidence.

Build caches, OEM modules, full runtime kallsyms dumps and device-specific
logs are intentionally excluded from this public-source copy.

## Prebuilt artifacts

```text
f7b5da52ca8ca138d33117788226c5d2fca3b8031a6f49fb85e3c33abd7b4ee1  artifacts/kernelsu-xpad2-4.19.191.ko
3145acec98ba2b31f9b376f50ad139bbab3efd812613d595e2328843382959e0  artifacts/ksud-xpad2
```

The same `.ko` is embedded in `ksud-xpad2` under KMI name
`xpad2-4.19.191`.

## Build prerequisites

You need:

- the matching XPad2 vendor kernel source/configuration;
- Android clang `r383902`;
- an x86_64 Linux build environment;
- Rust with the `aarch64-linux-android` target;
- Android NDK and `cargo-ndk`.

The local validation used a Lima x86_64 VM because the OEM compiler is an
x86_64 Linux binary. Paths below are examples; adjust them to your tree.

## Build the kernel module

```sh
export PATH=/path/to/clang-r383902/bin:$PATH

make -C /path/to/vendor-kernel \
  O=/path/to/kernel-out \
  M="$PWD/kernel" \
  ARCH=arm64 \
  CROSS_COMPILE=aarch64-linux-gnu- \
  CLANG_TRIPLE=aarch64-linux-gnu- \
  LLVM_IAS=1 \
  CC=clang LD=ld.lld AR=llvm-ar NM=llvm-nm \
  OBJCOPY=llvm-objcopy OBJDUMP=llvm-objdump STRIP=llvm-strip \
  CONFIG_KSU=m \
  CONFIG_KSU_LEGACY_4_19=y \
  CONFIG_KSU_DISABLE_MANAGER=y \
  KCFLAGS="-Wno-strict-prototypes -Wno-int-conversion \
    -Wno-gcc-compat -Wno-missing-prototypes \
    -Wno-declaration-after-statement -Wno-unused-function" \
  modules -j"$(nproc)"
```

Before building ksud, copy the resulting module to the embedded asset:

```sh
install -m 0644 kernel/kernelsu.ko \
  userspace/ksud/bin/aarch64/xpad2-4.19.191_kernelsu.ko
```

## Build ksud

```sh
export ANDROID_NDK_HOME=/path/to/android-ndk

cargo ndk -t arm64-v8a check -p ksud
cargo ndk -t arm64-v8a clippy -p ksud -- -D warnings
cargo fmt --all -- --check
cargo ndk -t arm64-v8a build --release -p ksud
```

The result is
`target/aarch64-linux-android/release/ksud`.

## Device use

You must already have authorized temporary root capable of loading a kernel
module. Push ksud and invoke the exact KMI:

```sh
adb push artifacts/ksud-xpad2 /data/local/tmp/ksud-xpad2
adb shell chmod 755 /data/local/tmp/ksud-xpad2

adb shell /path/to/temporary-su -c \
  "/data/local/tmp/ksud-xpad2 late-load --kmi xpad2-4.19.191"
```

Do not pass `--allow-shell`: the XPad2 legacy module uses a guarded,
hard-coded shell policy and does not expose that module parameter.

Validate:

```sh
adb shell /data/local/tmp/ksud-xpad2 debug info
adb shell 'echo "id; exit" | /data/local/tmp/ksud-xpad2 debug su'
```

Expected core fields:

```text
version: 32547
uapi_version: 2
lkm: true
late_load: true
runtime_mode: late-load
```

## Unload

Both paths were validated without changing the boot ID:

```sh
# Preferred userspace path for this exact target:
adb shell /path/to/temporary-su -c \
  "/data/local/tmp/ksud-xpad2 unload"

# Direct kernel-module path:
adb shell /path/to/temporary-su -c "rmmod kernelsu"
```

The XPad2-specific `ksud unload` path deliberately avoids Android's global
`stop`/`start` sequence, so it does not stop adbd or zygote.

## Important implementation notes

The port includes target-guarded solutions for:

- Linux 4.19 API and feature gaps;
- dynamic `sys_call_table` discovery from `el0_svc`;
- live TTBR1 page-table walking and fixmap writes with readback;
- direct legacy syscall adapters with preserved originals;
- reboot-supercall installation of the anonymous KSU driver fd;
- syscall adapter quiescing during unload;
- kprobe registration-state tracking;
- safe module kobject lifetime during unload.

See `PROGRESS.md` for the detailed failure analysis and hardware evidence.

## Safety and scope

Loading kernel modules and modifying syscall dispatch can crash the device or
corrupt data. Keep a recovery path and backups. Use only on hardware you own
or are explicitly authorized to test. This project is provided without
warranty.

## License and attribution

KernelSU and this derivative are distributed under GPL-2.0; see `LICENSE` and
the per-file SPDX headers. Upstream copyrights and history are retained in
the repository history.
