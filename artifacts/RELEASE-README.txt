XPad2 KernelSU late-load v0.2.1

Exact supported target:
  device: ls12_mt8797_wifi_64 (TALIH_PD2)
  Android: 13
  kernel: 4.19.191+
  KMI: xpad2-4.19.191

The current generic artifacts are Manager-enabled. ksud-xpad2 embeds the
matching kernelsu.ko. After obtaining authorized temporary root, push
ksud-xpad2 and run:

  ksud-xpad2 late-load --kmi xpad2-4.19.191

v0.2.1 restores the standard KernelSU SELinux domain on the legacy 4.19
late-load path and preserves a trusted Manager bootstrap fd across exec. The
official 32547 Manager was cold-started successfully; its root shell ran as
u:r:ksu:s0 while global SELinux remained Enforcing.

Early Manager pinning is enabled only when the installed official package
certificate matches the expected identity. A missing or untrusted Manager is
non-blocking: KernelSU still loads, but no Manager appId is pinned early.

Do not use these binaries on unrelated models, kernels, or firmware builds.
Verify every downloaded file against SHA256SUMS before use.

The Manager APK is not bundled, installed, or replaced. The files carrying
"no-manager" preserve the v0.1.0 build for provenance and are not the product
default. Do not unload or replace either module in a live boot. Leave a loaded
module resident until an ordinary reboot.

Project and full documentation:
  https://github.com/yoyicue/xpad2-ksu-lateload

Temporary-root entry point for the verified firmware:
  https://github.com/yoyicue/xpad2-ionstack-poc
