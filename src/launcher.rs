// LNP/BP Core Library implementing LNPBP specifications & standards
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use std::fmt::{self, Debug, Display};
use std::process::{Command, ExitStatus};
use std::thread::JoinHandle;
use std::{process, thread};

use amplify::IoError;
use internet2::transport;

/// Handle for a launched daemon/service
#[derive(Debug)]
pub enum DaemonHandle<DaemonName: Launcher> {
    /// Daemon launched as a separate process
    Process(DaemonName, process::Child),

    /// Daemon launched as a thread
    Thread(DaemonName, thread::JoinHandle<Result<(), DaemonName::RunError>>),
}

impl<DaemonName: Launcher> Display for DaemonHandle<DaemonName> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonHandle::Process(name, child) => write!(f, "{} PID #{}", name, child.id()),
            DaemonHandle::Thread(name, handle) => {
                write!(f, "{} {:?}", name, handle.thread().id())
            }
        }
    }
}

/// Errors during daemon launching
#[derive(Debug, Error, Display, From)]
#[display(doc_comments)]
pub enum LauncherError<DaemonName: Launcher> {
    /// Tor is not yet supported
    #[from(transport::Error)]
    TorNotSupportedYet,

    /// thread `{0}` has exited with an error.
    ///
    /// Error details: {1}
    ThreadAborted(DaemonName, DaemonName::RunError),

    /// thread `{0}` failed to launch due to I/O error {1}
    ThreadLaunch(DaemonName, IoError),

    /// thread `{0}` failed to launch
    ThreadJoin(DaemonName),

    /// process `{0}` has existed with a non-zero exit status {1}
    ProcessAborted(DaemonName, ExitStatus),

    /// process `{0}` failed to launch due to I/O error {1}
    ProcessLaunch(DaemonName, IoError),
}

impl<DaemonName: Launcher> DaemonHandle<DaemonName> {
    /// Waits for daemon execution completion on the handler.
    ///
    /// # Returns
    ///
    /// On error or upon thread/process successful completion. For process this means that the
    /// process has exited with status 0.
    ///
    /// # Errors
    /// - if the thread failed to start;
    /// - if it failed to join the thread;
    /// - if the process exit status was not 0
    pub fn join(self) -> Result<(), LauncherError<DaemonName>> {
        match self {
            DaemonHandle::Process(name, mut proc) => proc
                .wait()
                .map_err(|io| LauncherError::ProcessLaunch(name.clone(), io.into()))
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(LauncherError::ProcessAborted(name, status))
                    }
                }),
            DaemonHandle::Thread(name, thread) => thread
                .join()
                .map_err(|_| LauncherError::ThreadJoin(name.clone()))?
                .map_err(|err| LauncherError::ThreadAborted(name, err)),
        }
    }
}

pub trait Launcher: Clone + Debug + Display + Send + 'static {
    type RunError: std::error::Error + Send + 'static;
    type Config: Send + 'static;

    fn bin_name(&self) -> &'static str;

    fn cmd_args(&self, cmd: &mut Command) -> Result<(), LauncherError<Self>>;

    fn run_impl(self, config: Self::Config) -> Result<(), Self::RunError>;

    fn thread_daemon(
        self,
        config: Self::Config,
    ) -> Result<DaemonHandle<Self>, LauncherError<Self>> {
        debug!("Spawning {} as a new thread", self);

        let name = self.to_string();
        let d = self.clone();
        thread::Builder::new()
            .name(self.to_string())
            .spawn(move || match d.run_impl(config) {
                Ok(_) => unreachable!("daemons should never terminate by themselves"),
                Err(err) => {
                    error!("Daemon {} crashed: {}", name, err);
                    Err(err)
                }
            })
            .map_err(|io| LauncherError::ThreadLaunch(self.clone(), io.into()))
            .map(|handle: JoinHandle<Result<(), _>>| DaemonHandle::Thread(self, handle))
    }

    fn exec_daemon(self) -> Result<DaemonHandle<Self>, LauncherError<Self>> {
        let mut bin_path = std::env::current_exe().map_err(|err| {
            error!("Unable to detect binary directory: {}", err);
            LauncherError::ProcessLaunch(self.clone(), err.into())
        })?;
        bin_path.pop();
        bin_path.push(self.bin_name());
        #[cfg(target_os = "windows")]
        bin_path.set_extension("exe");

        debug!("Launching {} as a separate process using `{}` as binary", self, bin_path.display());

        let mut cmd = process::Command::new(bin_path);
        self.cmd_args(&mut cmd)?;

        trace!("Executing `{:?}`", cmd);
        cmd.spawn()
            .map_err(|err| {
                error!("Error launching {}: {}", self, err);
                LauncherError::ProcessLaunch(self.clone(), err.into())
            })
            .map(|process| DaemonHandle::Process(self, process))
    }
}
