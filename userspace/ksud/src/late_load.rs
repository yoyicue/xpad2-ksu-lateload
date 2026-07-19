use anyhow::{Context, Result, ensure};
use log::{info, warn};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::module::{handle_updated_modules, prune_modules};
use crate::{assets, defs, init_event, metamodule, restorecon, utils};

const OFFICIAL_MANAGER_CERT_SIZE: u32 = 0x033b;
const OFFICIAL_MANAGER_CERT_SHA256: &str =
    "c371061b19d8c7d7d6133c6a9bafe198fa944e50c1b31c9d8daa8d7f1fc2d2d6";

const XPAD3_TRACE_ROOT: &str = "/data/local/tmp/.xpad3/logs";
const XPAD3_TRACE_PROTOCOL: &str = "XPAD3_KSU_TRACE_V1";
const ANDROID_SHELL_UID: u32 = 2000;

struct LateLoadTrace {
    file: Option<File>,
    boot_id: String,
}

impl LateLoadTrace {
    fn open(path: Option<&Path>) -> Self {
        let mut trace = Self {
            file: None,
            boot_id: std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
                .unwrap_or_else(|_| "unknown".to_string())
                .trim()
                .to_string(),
        };
        let Some(path) = path else {
            return trace;
        };
        if !path.is_absolute() || !path.starts_with(XPAD3_TRACE_ROOT) {
            warn!(
                "ignoring late-load trace outside the xpad3 transaction log root: {}",
                path.display()
            );
            return trace;
        }
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                warn!("cannot inspect late-load trace {}: {error}", path.display());
                return trace;
            }
        };
        if !trace_metadata_is_safe(&metadata) {
            warn!(
                "refusing unsafe late-load trace {}; expected a shell-owned 0600 regular file",
                path.display()
            );
            return trace;
        }
        let file = match OpenOptions::new()
            .append(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
        {
            Ok(file) => file,
            Err(error) => {
                warn!("cannot open late-load trace {}: {error}", path.display());
                return trace;
            }
        };
        match file.metadata() {
            Ok(metadata) if trace_metadata_is_safe(&metadata) => trace.file = Some(file),
            Ok(_) => warn!(
                "refusing changed late-load trace {}; safety check no longer matches",
                path.display()
            ),
            Err(error) => warn!(
                "cannot revalidate late-load trace {}: {error}",
                path.display()
            ),
        }
        trace
    }

    fn record(&mut self, stage: &str, detail: Option<&str>) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let result = (|| -> std::io::Result<()> {
            let record = serde_json::json!({
                "ts": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "boot_id": self.boot_id,
                "pid": std::process::id(),
                "protocol": XPAD3_TRACE_PROTOCOL,
                "source": "ksud",
                "stage": stage,
                "detail": detail,
            });
            let mut line = serde_json::to_vec(&record).map_err(std::io::Error::other)?;
            line.push(b'\n');
            file.write_all(&line)?;
            file.flush()?;
            file.sync_data()
        })();
        if let Err(error) = result {
            warn!("disabling late-load trace after durable write failed: {error}");
            self.file = None;
        }
    }
}

fn trace_metadata_is_safe(metadata: &std::fs::Metadata) -> bool {
    metadata.is_file()
        && metadata.uid() == ANDROID_SHELL_UID
        && metadata.mode() & 0o777 == 0o600
        && metadata.nlink() == 1
}

fn get_manager_appid(package_name: &str) -> Result<u32> {
    ensure!(
        package_name == defs::DEFAULT_PACKAGE_NAME,
        "refusing to pin an untrusted Manager package: {package_name}"
    );

    let output = Command::new("/system/bin/pm")
        .args(["path", package_name])
        .output()
        .context("query installed Manager APK")?;
    ensure!(
        output.status.success(),
        "PackageManager cannot resolve the installed Manager"
    );
    let output = String::from_utf8(output.stdout).context("invalid PackageManager output")?;
    let apk = output
        .lines()
        .filter_map(|line| line.strip_prefix("package:"))
        .find(|path| path.ends_with("/base.apk"))
        .context("PackageManager returned no Manager base APK")?;
    let (cert_size, cert_sha256) = crate::apk_sign::get_apk_signature(apk)
        .with_context(|| format!("verify installed Manager signature at {apk}"))?;
    ensure!(
        cert_size == OFFICIAL_MANAGER_CERT_SIZE && cert_sha256 == OFFICIAL_MANAGER_CERT_SHA256,
        "installed Manager signature is not trusted"
    );

    let uid = rustix::fs::stat(format!("/data/data/{package_name}"))
        .with_context(|| format!("stat /data/data/{package_name}"))?
        .st_uid as u32;
    let appid = uid % 100_000;
    ensure!(appid != 0, "invalid Manager appId derived from uid {uid}");
    Ok(appid)
}

fn dump_process_info(label: &str) {
    use rustix::process::{getgid, getgroups, getpid, getuid};

    let pid = getpid().as_raw_nonzero();
    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    let groups: Vec<String> = getgroups()
        .unwrap_or_default()
        .iter()
        .map(|g| g.as_raw().to_string())
        .collect();
    let selinux = std::fs::read_to_string("/proc/self/attr/current")
        .unwrap_or_else(|_| "unknown".to_string());
    let seccomp = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Seccomp:"))
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        "[{label}] pid={pid}, uid={uid}, gid={gid}, groups=[{}], selinux={}, {seccomp}",
        groups.join(","),
        selinux.trim(),
    );
}

pub fn run(
    package_name: &String,
    kmi: Option<String>,
    allow_shell: bool,
    trace_path: Option<&Path>,
) -> Result<()> {
    let mut trace = LateLoadTrace::open(trace_path);
    trace.record("daemonize-enter", None);
    if let Err(error) = utils::daemonize(false) {
        trace.record("failed", Some(&format!("daemonize: {error:#}")));
        return Err(error);
    }
    trace.record("daemonized", None);
    let result = run_inner(package_name, kmi, allow_shell, &mut trace);
    match &result {
        Ok(()) => trace.record("complete", None),
        Err(error) => trace.record("failed", Some(&format!("{error:#}"))),
    }
    result
}

fn run_inner(
    package_name: &String,
    kmi: Option<String>,
    allow_shell: bool,
    trace: &mut LateLoadTrace,
) -> Result<()> {
    info!("late-load command triggered!");
    dump_process_info("late-load start");

    // 1. Check if KernelSU is already loaded
    if ksuinit::has_kernelsu() {
        info!("KernelSU already loaded, skip loading ko");
        trace.record("module-already-loaded", None);
    } else {
        // 2. Detect current KMI version
        let kmi = kmi.map_or_else(
            || crate::boot_patch::get_current_kmi().context("Failed to detect current KMI version"),
            Ok,
        )?;
        info!("Detected KMI: {kmi}");
        trace.record("kmi-selected", Some(&kmi));

        // 3. Get kernelsu.ko from embedded assets
        let ko_name = format!("{kmi}_kernelsu.ko");
        let ko_data = assets::get_asset_data(&ko_name)
            .with_context(|| format!("Failed to get {ko_name} from assets"))?;
        trace.record("module-asset-ready", Some(&ko_name));

        // 4. Load kernelsu.ko from memory with manual relocation
        let manager_appid = match get_manager_appid(package_name) {
            Ok(appid) => {
                info!("Verified official Manager appId {appid} for early pinning");
                Some(appid)
            }
            Err(error) => {
                warn!("Manager early pin unavailable; continuing without it: {error:#}");
                None
            }
        };
        trace.record(
            "manager-check-complete",
            Some(if manager_appid.is_some() {
                "official Manager pinned"
            } else {
                "Manager pin unavailable"
            }),
        );
        info!("Loading kernelsu.ko for KMI {kmi}...");
        let mut params = Vec::new();
        if allow_shell {
            params.push("allow_shell=1".to_string());
        }
        if let Some(appid) = manager_appid {
            params.push(format!("manager_appid={appid}"));
        }
        let params = params.join(" ");
        let params = CString::new(params).context("invalid module parameters")?;
        trace.record("init-module-enter", Some(&kmi));
        let load_result = ksuinit::load_module(&ko_data, params.as_c_str());
        trace.record(
            "init-module-returned",
            Some(if load_result.is_ok() {
                "success"
            } else {
                "error"
            }),
        );
        load_result.context("Failed to load kernelsu.ko")?;
        info!("kernelsu.ko loaded successfully!");
        dump_process_info("after load_module");
    }

    // We need to reset stdin/stdout/stderr; otherwise, sending file descriptors via cmd transactions
    // will be blocked by SELinux because its fsec->sid is still u:r:su:s0 instead of u:r:ksu:s0.
    utils::reset_std()?;
    trace.record("stdio-reset", None);

    utils::umask(0);

    if let Err(e) = crate::module_config::clear_all_temp_configs() {
        warn!("clear temp configs failed: {e}");
    }

    utils::install(None, None).context("Failed to install ksud")?;
    trace.record("userspace-installed", None);

    // 5. Handle module updates
    if let Err(e) = handle_updated_modules() {
        warn!("handle updated modules failed: {e}");
    }

    if let Err(e) = prune_modules() {
        warn!("prune modules failed: {e}");
    }

    if let Err(e) = restorecon::restorecon() {
        warn!("restorecon failed: {e}");
    }

    // 6. Load SELinux rules
    if crate::module::load_sepolicy_rule().is_err() {
        warn!("load sepolicy.rule failed");
    }

    if let Err(e) = crate::profile::apply_sepolies() {
        warn!("apply root profile sepolicy failed: {e}");
    }
    trace.record("sepolicy-complete", None);

    // 7. Initialize features
    if let Err(e) = crate::feature::init_features() {
        warn!("init features failed: {e}");
    }
    trace.record("features-complete", None);

    // 8. Execute late-load stage scripts (blocking)
    init_event::run_stage("late-load", true);
    trace.record("late-load-scripts-complete", None);

    // 9. Load system.prop
    if let Err(e) = crate::module::load_system_prop() {
        warn!("load system.prop failed: {e}");
    }

    // 10. Execute metamodule mount script (OverlayFS)
    if let Err(e) = metamodule::exec_mount_script(defs::MODULE_DIR) {
        warn!("execute metamodule mount failed: {e}");
    }

    // 11. Execute post-mount stage scripts (blocking)
    init_event::run_stage("post-mount", true);
    trace.record("post-mount-complete", None);

    // 12. Execute service stage scripts (non-blocking)
    init_event::run_stage("service", false);
    trace.record("service-started", None);

    // 13. Execute boot-completed stage scripts (non-blocking)
    init_event::run_stage("boot-completed", false);
    trace.record("boot-completed-started", None);

    // 14. Restart Manager so it gets a fresh ksu fd from the newly loaded kernel module
    info!("Restarting KernelSU Manager {package_name}...");
    let _ = Command::new("am")
        .args(["force-stop", package_name])
        .status();
    let _ = Command::new("am")
        .args([
            "start",
            "-n",
            &format!("{package_name}/me.weishu.kernelsu.ui.MainActivity"),
        ])
        .status();
    trace.record("manager-restart-requested", None);

    Ok(())
}
