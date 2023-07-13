use color_eyre::owo_colors::OwoColorize;
use std::process::exit;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{debug, info};

// TODO: How do the tasks ask if the server has exited? And better yet, how do they get the message back?
// TODO: Also, how do the tasks know when it has caused new stdout/stderr output?
// TODO: Allow the user to specify where to write the stdout/stderr of the monitored process. Maybe gzip compress it?
// TODO: Ask threads what their last packets were and dump it.
/// Start the broker process and monitor it. If it crashes, we stop our execution.
pub async fn start_supervised_process() -> color_eyre::Result<()> {
    let mut child = Command::new("nanomq")
        .arg("start")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to execute process");
    assert!(child.id().is_some());
    debug!("Started broker process");
    // Now broker should take longer than 2 seconds to start. But we could make this configurable.
    sleep(tokio::time::Duration::from_secs(2)).await;
    let mut stdout_reader = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr_reader = BufReader::new(child.stderr.take().unwrap()).lines();
    tokio::spawn(async move {
        loop {
            let last_stdout = stdout_reader.next_line().await.unwrap();
            let last_stderr = stderr_reader.next_line().await.unwrap();
            let status = child.try_wait();
            if let Ok(Some(status)) = status {
                info!(
                    "exited with: {}. Here is the last stdout and stderr",
                    status
                );
                if let Some(last_stdout) = last_stdout {
                    info!("{}", last_stdout);
                }
                if let Some(last_stderr) = last_stderr {
                    info!("{}", last_stderr.red());
                }
                exit(-1);
                break;
            }
            sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}
