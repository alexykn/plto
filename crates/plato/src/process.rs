use anyhow::{Context, Result, bail};
use std::process::{Child, Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub(crate) fn spawn(command: &mut Command, description: &str) -> Result<Child> {
    configure_process_group(command);
    command
        .spawn()
        .with_context(|| format!("Failed to start {description}"))
}

pub(crate) fn run_status(
    command: &mut Command,
    timeout: Duration,
    description: &str,
) -> Result<ExitStatus> {
    let mut child = spawn(command, description)?;
    let status = wait_with_timeout(&mut child, timeout, description)?;
    if !status.success() {
        bail!("{description} failed with status {status}");
    }
    Ok(status)
}

pub(crate) fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
    description: &str,
) -> Result<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("Failed while waiting for {description}"))?
        {
            return Ok(status);
        }
        if started.elapsed() >= timeout {
            terminate_process_tree(child, description)?;
            bail!(
                "{description} timed out after {} seconds",
                timeout.as_secs()
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn terminate_process_tree(child: &mut Child, description: &str) -> Result<()> {
    #[cfg(unix)]
    {
        let process_group = i32::try_from(child.id())
            .context("Child ID cannot be represented as a process group")?;
        let process_group = process_group
            .checked_neg()
            .context("Child process group cannot be negated")?;
        let result = unsafe { libc::kill(process_group, libc::SIGKILL) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error).with_context(|| {
                    format!("Failed to terminate process group for {description}")
                });
            }
        }
    }

    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .status()
            .with_context(|| format!("Failed to terminate process tree for {description}"))?;
        if !status.success() {
            bail!("Failed to terminate process tree for {description}: {status}");
        }
    }

    #[cfg(all(not(unix), not(windows)))]
    child
        .kill()
        .with_context(|| format!("Failed to terminate {description}"))?;

    child
        .wait()
        .with_context(|| format!("Failed to reap timed out {description}"))?;
    Ok(())
}

fn configure_process_group(command: &mut Command) {
    #[cfg(unix)]
    command.process_group(0);

    #[cfg(windows)]
    {
        command.creation_flags(0x0000_0200);
    }

    #[cfg(all(not(unix), not(windows)))]
    let _ = command;
}
