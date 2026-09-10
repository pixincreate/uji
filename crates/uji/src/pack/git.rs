use std::path::Path;
use std::process::Command;

pub(crate) fn run(cwd: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    let detail = if detail.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        detail.to_string()
    };
    Err(format!("git {}: {detail}", args.join(" ")))
}

pub(crate) fn clone(url: &str, dir: &Path, reference: Option<&str>) -> Result<(), String> {
    let target = dir.display().to_string();
    match reference {
        Some(reference) => {
            run(None, &["clone", url, &target])?;
            run(Some(dir), &["checkout", "--detach", reference])?;
        }
        None => {
            run(None, &["clone", "--depth", "1", url, &target])?;
        }
    }
    Ok(())
}

pub(crate) fn head(dir: &Path) -> Result<String, String> {
    run(Some(dir), &["rev-parse", "HEAD"])
}

fn remote_target(dir: &Path, reference: Option<&str>) -> Result<String, String> {
    let Some(reference) = reference else {
        return run(Some(dir), &["rev-parse", "--abbrev-ref", "origin/HEAD"]).or_else(|_| {
            run(
                Some(dir),
                &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
            )
        });
    };
    let remote = format!("origin/{reference}");
    let probe = format!("{remote}^{{commit}}");
    if run(Some(dir), &["rev-parse", "--verify", "--quiet", &probe]).is_ok() {
        return Ok(remote);
    }
    Ok(reference.to_string())
}

pub(crate) fn update(dir: &Path, reference: Option<&str>) -> Result<String, String> {
    run(Some(dir), &["fetch", "--tags", "--force", "origin"])?;
    let target = remote_target(dir, reference)?;
    run(Some(dir), &["reset", "--hard", &target])?;
    head(dir)
}
