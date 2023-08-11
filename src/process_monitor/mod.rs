use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast::Sender;
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

// TODO: How do the tasks ask if the server has exited? And better yet, how do they get the message back?
// TODO: Also, how do the tasks know when it has caused new stdout/stderr output?
// TODO: Allow the user to specify where to write the stdout/stderr of the monitored process. Maybe gzip compress it?
// TODO: Ask threads what their last packets were and dump it.
/// Start the broker process and monitor it. If it crashes, we stop our execution.
pub async fn start_supervised_process(
    sender: Sender<()>,
    command: String,
) -> color_eyre::Result<()> {
    let mut child = Command::new("/bin/sh")
        .args(["-c", &command])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to execute process");
    assert!(child.id().is_some());
    debug!("Started broker process");
    // Now broker should take longer than 2 seconds to start. But we could make this configurable.
    sleep(tokio::time::Duration::from_secs(1)).await;
    let mut stdout_reader = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr_reader = BufReader::new(child.stderr.take().unwrap()).lines();
    tokio::spawn(async move {
        let mut last_stdout: String = String::new();
        let mut last_stderr: String = String::new();
        loop {
            if let Ok(Ok(Some(new_stdout))) =
                timeout(Duration::from_millis(100), stdout_reader.next_line()).await
            {
                last_stdout.push('\n');
                last_stdout.push_str(new_stdout.as_str());
            }
            if let Ok(Ok(Some(new_stderr))) =
                timeout(Duration::from_millis(100), stderr_reader.next_line()).await
            {
                last_stderr.push('\n');
                last_stderr.push_str(new_stderr.as_str());
            }
            let status = child.try_wait();
            if let Ok(Some(status)) = status {
                sender.send(()).unwrap();
                info!("Broker process exited with status: {}", status);
                info!("Stdout: {:?}", last_stdout);
                info!("Stderr: {:?}", last_stderr);
                break;
            }
        }
    });
    Ok(())
}
