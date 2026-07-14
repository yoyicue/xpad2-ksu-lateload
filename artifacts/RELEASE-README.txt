XPad2 KernelSU late-load v0.2.0

Exact supported target:
  device: ls12_mt8797_wifi_64 (TALIH_PD2)
  Android: 13
  kernel: 4.19.191+
  KMI: xpad2-4.19.191

The current generic artifacts are Manager-enabled. ksud-xpad2 embeds the
matching kernelsu.ko. After obtaining authorized temporary root, push
ksud-xpad2 and run:

  ksud-xpad2 late-load --kmi xpad2-4.19.191

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
