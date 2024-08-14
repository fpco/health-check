use nix::sys::signal::Signal;
use parking_lot::Mutex;
use reqwest::Url;
use signal_hook::consts::{SIGINT, SIGTERM};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use clap::{arg, Parser};

use crate::{line_helper::LineHelper, slack::SlackApp};

#[derive(Parser)]
pub(crate) struct Cli {
    /// Seconds to wait for output before killing the task
    #[arg(long)]
    pub(crate) task_output_timeout: Option<u64>,
    /// Slack Webhook for notification
    #[arg(long, value_parser(Url::from_str), env = "HEALTH_CHECK_SLACK_WEBHOOK")]
    pub(crate) slack_webhook: Url,
    /// Application description
    #[arg(long)]
    pub(crate) app_description: String,
    /// Application version
    #[arg(long, env = "HEALTH_CHECK_APP_VERSION")]
    pub(crate) app_version: String,
    /// Notification Context
    #[arg(long, env = "HEALTH_CHECK_NOTIFICATION_CONTEXT")]
    pub(crate) notification_context: String,
    /// Image url for notification message
    #[arg(long, env = "HEALTH_CHECK_IMAGE_URL", required = false)]
    pub(crate) image_url: Option<String>,
    /// Is the child process allowed to exit on its own? By default it is false.
    #[arg(long)]
    can_exit: bool,
    /// Process to run
    #[arg(required = true)]
    pub(crate) command: String,
    /// Arguments to the process
    #[arg(required = false)]
    pub(crate) args: Vec<String>,
    /// How many lines of output should we store for error messages?
    #[arg(long, default_value_t = 50, env = "HEALTH_CHECK_OUTPUT_LINES")]
    pub(crate) output_lines: usize,
}

#[derive(Debug)]
enum MainMessage {
    Error(anyhow::Error),
    DeadlockDetected,
    ChildExited(ExitStatus),
}

#[derive(Clone)]
struct SendMainMessage(mpsc::Sender<MainMessage>);

impl SendMainMessage {
    fn send(&self, msg: MainMessage) {
        if let Err(err) = self.0.send(msg) {
            eprintln!(
                "Unable to send MainMessage, looks like we're already shutting down: {:?}",
                err.0
            );
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum StdType {
    Stdout,
    Stderr,
}

impl Cli {
    pub(crate) fn run(self) -> Result<()> {
        let mut command = Command::new(&self.command);
        command.args(&self.args[..]);

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .context(format!("Failed to spawn {}", self.command))?;

        let (send, recv) = mpsc::channel::<MainMessage>();
        let send = SendMainMessage(send);
        let max_recent_output = self.output_lines;
        let recent_output = Arc::new(Mutex::new(VecDeque::with_capacity(max_recent_output)));

        // Always capture output so we can keep recent output available for error messages.
        let last_output = Arc::new(Mutex::new(Instant::now()));
        {
            let child_stdout = child.stdout.take().context("child stdout is None")?;
            let child_stderr = child.stderr.take().context("child stderr is None")?;
            let send_clone = send.clone();
            let last_output_clone = last_output.clone();
            let recent_output_clone = recent_output.clone();
            std::thread::spawn(move || {
                process_std_handle(
                    child_stdout,
                    send_clone,
                    StdType::Stdout,
                    last_output_clone,
                    recent_output_clone,
                    max_recent_output,
                )
            });
            let send_clone = send.clone();
            let last_output_clone = last_output.clone();
            let recent_output_clone = recent_output.clone();
            std::thread::spawn(move || {
                process_std_handle(
                    child_stderr,
                    send_clone,
                    StdType::Stderr,
                    last_output_clone,
                    recent_output_clone,
                    max_recent_output,
                )
            });
        }

        if let Some(task_output_timeout) = self.task_output_timeout {
            let send_clone = send.clone();
            std::thread::spawn(move || {
                detect_deadlock(
                    last_output,
                    send_clone,
                    Duration::from_secs(task_output_timeout),
                )
            });
        }

        let child_pid = i32::try_from(child.id())?;
        static CHILD_WAS_KILLED: AtomicBool = AtomicBool::new(false);
        std::thread::spawn({
            let send = send.clone();
            move || {
                handle_signals(
                    send,
                    nix::unistd::Pid::from_raw(child_pid),
                    &CHILD_WAS_KILLED,
                )
            }
        });

        std::thread::spawn(|| watch_child(send, child));

        let msg = recv.recv();
        // Drop the recv immediately, just a minor optimization to avoid
        // additional messages building up in the queue where we won't see them.
        std::mem::drop(recv);
        let res = match msg {
            Ok(msg) => match msg {
                MainMessage::Error(e) => Err(e),
                MainMessage::DeadlockDetected => Err(anyhow::anyhow!(
                    "Potential deadlock detected, too long without output from child process"
                )),
                MainMessage::ChildExited(exit_status) => {
                    if self.can_exit && exit_status.success()
                        || CHILD_WAS_KILLED.load(Ordering::SeqCst)
                    {
                        eprintln!("Child exited, treating as a success case");
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Child exited with status {exit_status}"))
                    }
                }
            },
            Err(_) => Err(anyhow::anyhow!(
                "Impossible, all send channels have been closed"
            )),
        };

        match res {
            Ok(()) => Ok(()),
            Err(e) => {
                let slack_app = SlackApp::new(
                    self.slack_webhook,
                    self.notification_context,
                    self.app_description,
                    self.app_version,
                    self.image_url,
                );
                let mut msg = String::new();
                for line in &*recent_output.lock() {
                    msg.push_str(line);
                    msg.push('\n');
                }
                let result = slack_app.send_notification(&e, &msg);
                if let Err(err) = result {
                    eprintln!("Slack notification failed: {err:?}");
                }
                Err(e)
            }
        }
    }
}

fn process_std_handle(
    mut reader: impl Read,
    send: SendMainMessage,
    std_type: StdType,
    last_output: Arc<Mutex<Instant>>,
    recent_output: Arc<Mutex<VecDeque<String>>>,
    max_recent_output: usize,
) {
    let mut buffer = [0u8; 4096];
    let mut line_helper = LineHelper::new();

    loop {
        match reader
            .read(&mut buffer)
            .context("Unable to read from {std_type:?}")
        {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                *last_output.lock() = Instant::now();
                let buffer = &buffer[..size];
                let res = match std_type {
                    StdType::Stdout => std::io::stdout()
                        .lock()
                        .write_all(buffer)
                        .context("Unable to write to stdout"),
                    StdType::Stderr => std::io::stderr()
                        .lock()
                        .write_all(buffer)
                        .context("Unable to write to stderr"),
                };
                if let Err(e) = res {
                    send.send(MainMessage::Error(e));
                    break;
                }

                for line in line_helper.append(&buffer[..size]) {
                    let mut guard = recent_output.lock();
                    if guard.len() >= max_recent_output {
                        guard.pop_front();
                    }
                    guard.push_back(line);
                }
            }
            Err(e) => {
                send.send(MainMessage::Error(e));
                break;
            }
        }
    }

    if let Some(line) = line_helper.finish() {
        let mut guard = recent_output.lock();
        if guard.len() >= max_recent_output {
            guard.pop_front();
        }
        guard.push_back(line);
    }
}

fn detect_deadlock(
    last_output_mutex: Arc<Mutex<Instant>>,
    send: SendMainMessage,
    task_output_timeout: Duration,
) {
    loop {
        let last_output = *last_output_mutex.lock();
        let next_deadlock_detected = match last_output
            .checked_add(task_output_timeout)
            .context("Deadlock detection: overflowed Instant")
        {
            Ok(x) => x,
            Err(e) => {
                send.send(MainMessage::Error(e));
                break;
            }
        };
        match next_deadlock_detected.checked_duration_since(Instant::now()) {
            Some(to_sleep) => {
                std::thread::sleep(to_sleep);
            }
            None => {
                send.send(MainMessage::DeadlockDetected);
                break;
            }
        }
    }
}

fn watch_child(send: SendMainMessage, mut child: Child) {
    match child
        .wait()
        .context("Unable to wait for child process to exit")
    {
        Ok(exit_status) => send.send(MainMessage::ChildExited(exit_status)),
        Err(e) => send.send(MainMessage::Error(e)),
    }
}

fn handle_signals(
    send: SendMainMessage,
    child_pid: nix::unistd::Pid,
    child_was_killed: &AtomicBool,
) {
    let mut signals = match signal_hook::iterator::Signals::new([SIGTERM, SIGINT])
        .context("Creating new Signals value")
    {
        Ok(signals) => signals,
        Err(e) => {
            send.send(MainMessage::Error(e));
            return;
        }
    };

    for signal in signals.forever() {
        match Signal::try_from(signal)
            .with_context(|| format!("Unable to convert signal value for nix: {signal}"))
        {
            Ok(signal) => {
                child_was_killed.store(true, Ordering::SeqCst);
                if let Err(e) = nix::sys::signal::kill(child_pid, signal)
                    .context("Unable to send signal to child process")
                {
                    send.send(MainMessage::Error(e));
                }
            }
            Err(e) => {
                send.send(MainMessage::Error(e));
            }
        };
    }
}
