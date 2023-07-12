use color_eyre::owo_colors::OwoColorize;
use std::process::exit;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{debug, info};

// TODO: How do the tasks ask if the server has exited? And better yet, how do they get the message back?
// TODO: Also, how do the tasks know when it has caused new stdout/stderr output?
// TODO: Allow the user to specify where to write the stdout/stderr of the monitored process. Maybe gzip compress it?
/// Start the broker process and monitor it. If it crashes, we stop our execution.
pub async fn start_supervised_process() -> color_eyre::Result<()> {
    let mut child = Command::new("mosquitto")
        .arg("-v")
        .spawn()
        .expect("failed to execute process");
    assert!(child.id().is_some());
    debug!("Started broker process");
    // Now broker should take longer than 2 seconds to start. But we could make this configurable.
    sleep(tokio::time::Duration::from_secs(2)).await;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    tokio::spawn(async move {
        loop {
            //let stdout_buffer = &mut [0; 1024];
            //let _ = stdout.expect("STDOUT NOT FOUND").read(stdout_buffer).await;
            let stderr_buffer = &mut [0; 1024];
            if let Some(err) = stderr.as_mut() {
                let _ = err.read(stderr_buffer).await;
            }
            let status = child.try_wait();
            if let Ok(Some(status)) = status {
                info!(
                    "exited with: {}. Here is the last stdout and stderr",
                    status
                );
                //info!("stdout: {}", String::from_utf8_lossy(stdout_buffer));
                info!("stderr: {}", String::from_utf8_lossy(stderr_buffer).red());
                exit(-1);
                break;
            }
            sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}
