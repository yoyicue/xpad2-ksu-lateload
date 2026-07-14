# XPad2 KernelSU late-load port progress

> Historical lab journal. Commands and removal results below document past
> development only; they do not authorize current execution. Current policy
> forbids `ksud unload`, direct removal of `kernelsu` with `rmmod`, and
> same-boot module replacement. Leave a loaded module resident until an
> ordinary reboot.

Date: 2026-07-11

Target: XPad2 `ls12_mt8797_wifi_64`, Android 13 `/260`, Linux
`4.19.191+`, OEM clang `r383902`.

Pinned KernelSU revision:

```text
ccfee6dc67d79a1404eb71c67b67b4f6c7b65a4a
```

This is the first inspected official main revision containing the matching
`ksud late-load` command and its in-memory `ksuinit::load_module` path.

## Proven

- Exact-target hello LKM loads and unloads on the device.
- KernelSU main has been ported far enough to compile and link a
  `kernel/kernelsu.ko` for Linux 4.19 using the OEM config/toolchain.
- The module is built in an explicit legacy mode:
  - `CONFIG_KSU_LEGACY_4_19=y`
  - `CONFIG_KSU_DISABLE_MANAGER=y`
  - modern file-wrapper disabled/stubbed;
  - modern SELinux policy clone/replace and SELinux-hide disabled/stubbed;
  - legacy mode retains the exploit-provided permissive SELinux context;
  - missing 4.19 helper names/types have guarded compatibility shims.
- The resulting module has 146 undefined kernel symbols.
- Installed OEM module `__versions` records cover 52/146 with no CRC conflict.
- This is not the decisive constraint because `ksud late-load` uses
  `ksuinit::load_module`, which rewrites undefined ELF symbols to absolute
  addresses from `/proc/kallsyms` before `init_module`.
- Runtime kallsyms resolves 125/146 imported symbols.

## Current hard gate

The following 21 symbols are not visible in the runtime `/proc/kallsyms` and
therefore cannot be relocated by the official manual loader:

```text
__cpu_online_mask
__tracepoint_sys_enter
arm64_const_caps_ready
cpu_hwcap_keys
cpu_hwcaps
handle_sepolicy
init_mm
init_pid_ns
init_task
kmalloc_caches
memstart_addr
mntns_operations
param_ops_bool
path_mount
path_umount
seccomp_filter_release
selinux_state
static_key_initialized
system_wq
tasklist_lock
tracepoint_srcu
```

Most are data symbols hidden because this OEM kernel does not expose all data
symbols through kallsyms. `path_mount` and `path_umount` are newer APIs absent
from 4.19. Loading the module before removing/replacing all 21 dependencies is
not allowed.

## Next work

1. Eliminate `param_ops_bool` by hardcoding the legacy late-load shell policy
   and omitting legacy module parameters.
2. Stub the unused userspace sepolicy command (`handle_sepolicy`) in legacy
   mode.
3. Replace/disable `path_mount`, `path_umount`, mount-namespace and pre-5.9
   umount functionality for the first late-load core test.
4. Remove policy-only `selinux_state` and seccomp internals from the legacy
   permissive path.
5. Audit the ARM64 syscall hook implementation. Its patch-memory path pulls in
   `init_mm`, `memstart_addr`, CPU capability data, online-mask data and
   tracepoint state. Choose a 4.19-safe hook method that uses kallsyms-visible
   functions only; do not guess addresses for hidden data.
6. Rebuild and require 0 missing runtime symbols.
7. Build `ksud` from the same revision with the XPad2 module embedded as
   `4.19.191+_kernelsu.ko` (or a deliberately chosen `--kmi` asset name).
8. Only after static gates pass, run `ksud late-load` under the existing
   temporary root and verify KSU UAPI/root/unload behavior.

## Evidence

- `KernelSU/kernel/kernelsu.ko`
- `ksu_imports.txt`
- `ksu_imports_missing_runtime.txt`
- `runtime_kallsyms.txt`
- `oem_modules/`
- prior LKM ABI proof:
  `../xpad2_lkm_abi_20260711/VALIDATION_REPORT.md`

## Final device validation (2026-07-11)

The hard gate above is historical. The final legacy port has 93 imports and
zero names missing from the target runtime kallsyms. The final artifacts are:

```text
f7b5da52ca8ca138d33117788226c5d2fca3b8031a6f49fb85e3c33abd7b4ee1  kernel/kernelsu.ko
f7b5da52ca8ca138d33117788226c5d2fca3b8031a6f49fb85e3c33abd7b4ee1  userspace/ksud/bin/aarch64/xpad2-4.19.191_kernelsu.ko
3145acec98ba2b31f9b376f50ad139bbab3efd812613d595e2328843382959e0  target/aarch64-linux-android/release/ksud
```

The standalone and embedded module hashes are identical. On device boot ID
`8e17d437-33c4-4bd8-9920-ffc0dfa3e112`, with no KernelSU module initially
loaded, the following command was executed through the temporary root:

```sh
/data/local/tmp/ksud-xpad2 late-load --kmi xpad2-4.19.191
```

It returned zero. The boot ID did not change, `/proc/modules` showed
`kernelsu` live, and the same ksud then reported:

```text
version: 32547
flags: 0x5
uapi_version: 2
features: 0x5
lkm: true
late_load: true
runtime_mode: late-load
```

KernelSU's own `ksud debug su` (not the temporary exploit su) produced:

```text
uid=0(root) gid=0(root)
Uid: 0 0 0 0
Gid: 0 0 0 0
CapEff: 0000003fffffffff
```

Key ARM64 legacy fixes were: correct ADRP `immhi/immlo` decoding; walking the
live TTBR1 page tables to obtain the actual syscall-table physical page;
fixmap write/readback verification; a direct reboot supercall hook for the
anonymous KSU driver fd; and preserving original syscall functions so direct
table adapters do not recurse.

## Unload root cause and final validation (2026-07-12)

The unload reboot is fixed and its cause is definitive. Live dmesg showed all
five syscall entries restored with successful readback, all legacy adapters
drained, and the module listed as last unloaded. The subsequent fault was:

```text
pc: kernfs_remove_by_name_ns+0x78/0x90
```

The legacy module had called `kobject_del(&THIS_MODULE->mkobj.kobj)` during
init to hide itself. Normal module teardown then removed the same kernfs
object a second time. Legacy mode now leaves the module kobject registered so
the module core owns its single removal. In addition, the late-load
`input_event` kprobe is now unregistered according to recorded registration
state, and syscall adapters have an exit gate plus active-call drain.

On boot ID `88209a52-fc6f-48f5-a664-f2755c095b58`, direct `rmmod kernelsu`
returned zero, the module disappeared, the boot ID stayed unchanged, and the
reboot magic syscall again returned the original `EPERM`. A second late-load
succeeded in the same boot.

The stock unload command's global Android `stop`/`start` sequence was also
the reason it previously disconnected ADB and risked watchdog reboot. The
exact XPad2 legacy target now uses a direct unload path. Device validation
showed `ksud unload` returning zero while boot ID stayed unchanged, the module
was removed, and both `adbd` and `zygote` remained `running`. A subsequent
same-boot late-load restored KernelSU version 32547, UAPI 2, and working
`ksud debug su` root.

## Manager-enabled product artifacts (2026-07-14)

The initial v0.1.0 artifacts deliberately used
`CONFIG_KSU_DISABLE_MANAGER=y` while the Linux 4.19 compatibility boundary was
being established. The product build now enables Manager integration. Its
remaining compile blocker was the newer `fsnotify` inode-event callback; the
target-guarded legacy adapter now supplies Linux 4.19's `handle_event`
signature and routes both API variants through the same bounded
`packages.list` name check.

The Manager-enabled module and the module embedded in its matching ksud are
byte-identical:

```text
a99d230975c70c7efe72142770f936cd5e7585cfdd6ba9c8d45807bdf87b3f13  artifacts/kernelsu-xpad2-4.19.191.ko
8e6fed9f063b9b998f5b0cec8b64f31ad1eea885c528b9e9883ff3f4cd108e06  artifacts/ksud-xpad2
```

Offline `cargo fmt --check`, `cargo ndk check`, clippy with warnings denied,
and release build passed for the matching source tree. Physical-device
validation loaded the module from an initially unloaded boot, retained SELinux
enforcing state, exposed KernelSU version 32547/UAPI 2, and the already
installed `me.weishu.kernelsu` Manager displayed working state. The loader did
not install or replace the Manager APK.

For provenance, the v0.1.0 Manager-disabled binaries remain under explicit
`*-no-manager*` names. Current generic artifact names refer to the
Manager-enabled build. Neither variant may be removed or replaced online;
leave a loaded module resident until an ordinary reboot.
