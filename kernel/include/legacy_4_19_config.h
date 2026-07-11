#ifndef __KSU_LEGACY_4_19_CONFIG_H
#define __KSU_LEGACY_4_19_CONFIG_H

/* Override config-dependent header inlines for the late-load module only. */
#undef CONFIG_ARM64_LSE_ATOMICS
#undef CONFIG_HAVE_SYSCALL_TRACEPOINTS
#undef CONFIG_SLUB
#define CONFIG_SLOB 1

#endif
