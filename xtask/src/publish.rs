use anyhow::{bail, Result};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::transform::{CRATE_PUBLISH_ORDER, crate_name_from_path, unofficial_name};

/// crates.io allows a burst of 5 new crates, then 1 per 10 minutes.
/// For existing crates (version updates), the limit is more generous.
const NEW_CRATE_BURST: usize = 5;
/// Delay after the burst for new crates (10 min + buffer)
const NEW_CRATE_DELAY: Duration = Duration::from_secs(630);
/// Delay between publishes for propagation
const PROPAGATION_DELAY: Duration = Duration::from_secs(30);

fn crate_exists_on_registry(name: &str) -> bool {
    Command::new("cargo")
        .args(["search", name, "--limit", "1"])
        .output()
        .ok()
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains(name))
}

pub fn run(crates_dir: &str, dry_run: bool) -> Result<()> {
    let crates_path = Path::new(crates_dir);

    if !crates_path.exists() {
        bail!("Crates directory not found: {crates_dir}");
    }

    println!(
        "Publishing {} crates {}",
        CRATE_PUBLISH_ORDER.len(),
        if dry_run { "(dry run)" } else { "" }
    );

    let mut new_crate_count = 0;

    for (i, crate_entry) in CRATE_PUBLISH_ORDER.iter().enumerate() {
        let crate_name = crate_name_from_path(crate_entry);
        let pkg_name = unofficial_name(crate_name);
        let crate_path = crates_path.join(&pkg_name);

        if !crate_path.exists() {
            println!("Skipping {pkg_name} (not found)");
            continue;
        }

        // Check rate limiting for new crates
        if !dry_run {
            let is_new = !crate_exists_on_registry(&pkg_name);
            if is_new {
                new_crate_count += 1;
                if new_crate_count > NEW_CRATE_BURST {
                    println!(
                        "Rate limit: new crate #{new_crate_count} (past burst of {NEW_CRATE_BURST}), waiting {NEW_CRATE_DELAY:?}..."
                    );
                    thread::sleep(NEW_CRATE_DELAY);
                }
            }
        }

        println!(
            "[{}/{}] Publishing {pkg_name}...",
            i + 1,
            CRATE_PUBLISH_ORDER.len()
        );

        let mut cmd = Command::new("cargo");
        cmd.args(["publish", "--allow-dirty"]);

        if dry_run {
            cmd.arg("--dry-run");
        }

        cmd.current_dir(&crate_path);

        let status = cmd.status()?;

        if !status.success() {
            bail!("Failed to publish {pkg_name}");
        }

        // Wait for crates.io propagation (except for dry run or last crate)
        if !dry_run && i < CRATE_PUBLISH_ORDER.len() - 1 {
            println!("Waiting {PROPAGATION_DELAY:?} for crates.io propagation...");
            thread::sleep(PROPAGATION_DELAY);
        }
    }

    println!("\nPublish complete!");

    Ok(())
}
