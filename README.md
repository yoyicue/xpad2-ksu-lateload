# XPad2 / XPad3S KernelSU late-load ports

Experimental KernelSU late-load ports for the exact XPad2/TALIH PD2 and
XPad3S/TALIH PD3S firmware described in this repository. They load KernelSU at
runtime without replacing the boot image.

To obtain the temporary root required by the loader on the verified firmware,
see [yoyicue/xpad2-ionstack-poc](https://github.com/yoyicue/xpad2-ionstack-poc).
That project implements the pure-C, host-assisted re-root stage; this project
starts at the resulting authorized temporary-root boundary and installs the
runtime KernelSU module.

The XPad2 legacy path remains the release baseline documented below. The
separate modern GKI path and its physical-device evidence are documented in
[`XPAD3S.md`](XPAD3S.md).

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
- Manager-enabled package observation on Linux 4.19;
- the official production-signed `me.weishu.kernelsu` Manager reporting
  working state and KernelSU version 32547 after a cold restart;
- Manager bootstrap and root shells running in `u:r:ksu:s0` while global
  SELinux remains Enforcing;
- historical development validation also covered removal and same-boot reload.

The removal observations are retained only as historical evidence. Current
product and test policy forbids online removal or replacement of the loaded
module: do not execute `ksud unload` or remove `kernelsu` with `rmmod`. Leave
the module loaded until an ordinary reboot.

The current kernel module imports 137 runtime symbols, with zero missing from the
verified target's runtime kallsyms.

## Repository layout

- repository root: upstream KernelSU Git history plus the XPad2 port changes.
- `artifacts/`: the exact binaries used for final device validation.
- `PROGRESS.md`: development history, root causes and validation evidence.
- `XPAD3S.md`: Android 12 5.10 GKI build and XPad3S hardware evidence.

Build caches, OEM modules, full runtime kallsyms dumps and device-specific
logs are intentionally excluded from this public-source copy.

## Prebuilt artifacts

```text
e930a6929c6cd156f394e6b15bed2258b19205cc17fa3410db7f68cef7b8fb21  artifacts/kernelsu-xpad2-4.19.191.ko
26ea0f41af159a63a9afdff98963247da9d0bad0363f7e9c937f4cfbcd9f69c6  artifacts/ksud-xpad2
f7b5da52ca8ca138d33117788226c5d2fca3b8031a6f49fb85e3c33abd7b4ee1  artifacts/kernelsu-xpad2-4.19.191-no-manager.ko
3145acec98ba2b31f9b376f50ad139bbab3efd812613d595e2328843382959e0  artifacts/ksud-xpad2-no-manager
b481d4110ef60ddfa7ede99d165a87b7e0072a0aa5c11954dac89402273cfbd5  artifacts/kernelsu-xpad3s-android12-5.10.ko
9d3e66a4bced4327ae3db35df9425fd63af0b8cb3397f21dbfae287c1a3f40a2  artifacts/ksud-xpad3s
```

The same `.ko` is embedded in `ksud-xpad2` under KMI name
`xpad2-4.19.191`. The generic artifact names are the current Manager-enabled
build. The `*-no-manager*` pair preserves the v0.1.0 build for audit and
rollback comparison; it is not the product default.

No Manager APK is bundled or installed by these artifacts. The late-load
module only enables KernelSU's Manager integration; installation and package
identity remain separate operator decisions.

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
  KSU_VERSION=32547 \
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
export KSU_VERSION_CODE=32547
export KSU_VERSION_NAME=0.2.1-xpad2

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

## Historical removal evidence — do not execute

Earlier development runs validated both the userspace removal path and direct
kernel-module removal without changing the boot ID, followed by a same-boot
reload. Those results remain useful as implementation-history evidence, but
they are not current operating instructions.

The supported lifecycle is now load once and leave the module resident until
an ordinary reboot. If a loaded module is mismatched, unhealthy, or needs to
be replaced, stop and reboot; do not attempt online removal or replacement.

## Important implementation notes

The port includes target-guarded solutions for:

- Linux 4.19 API and feature gaps;
- a Linux 4.19 `fsnotify` event adapter for Manager package observation;
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
