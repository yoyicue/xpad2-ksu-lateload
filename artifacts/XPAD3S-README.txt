XPad3S KernelSU late-load experimental build

Exact verified target:
  device: TALIH-PD3S
  Android: 13 (TP1A.220624.014/338)
  kernel: 5.10.198-android12-9-00019-g6efebf1322d6-ab11471183
  KMI: android12-5.10

ksud-xpad3s embeds both android12-5.10 and xpad2-4.19.191 modules. For the
XPad3S, invoke it through an already-authorized temporary-root channel:

  ksud-xpad3s late-load --kmi android12-5.10 --allow-shell

The driver and ksud version are explicitly pinned to 32547 to match the
official production-signed Manager from upstream commit ccfee6dc. Do not
replace it with a Git-count-derived fork version.

The xpad3 control plane can additionally arm the hidden, default-off
XPAD3_KSU_TRACE_V1 stage trace for reboot diagnostics. Direct users do not
need to pass that internal argument.

Physical validation covered dynamic module loading, KernelSU root, the
official v3.2.5 Manager, and restoration of SELinux Enforcing. The change is
not persistent across reboot. Do not unload or replace a module in a live
boot; reboot before testing another build.

The Manager APK is not bundled. Verify these binaries against SHA256SUMS and
do not use them on an unrelated device, KMI, or firmware.
