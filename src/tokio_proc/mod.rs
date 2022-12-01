use std::fs::OpenOptions;
use std::io;
use std::io::ErrorKind;
use std::os::unix::prelude::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Stdio};

use nix::sys::signal::{kill, Signal};
use nix::sys::stat::Mode;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{mkfifo, Pid};
use tokio::fs::{File, metadata, remove_file};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};
use tokio::spawn;
use tokio::task::spawn_blocking;

pub struct ChildWrapper {
	handle: ChildHandle,
	pub stdin: Option<Box<dyn AsyncWrite + Unpin>>,
	pub stdout: Option<Box<dyn AsyncRead + Unpin>>,
	pub stderr: Option<Box<dyn AsyncRead + Unpin>>,
}

enum ChildHandle {
	Owned(Child),
	Attached(Pid),
}

impl ChildWrapper {
	/// Spawn command and make it re-attachable
	/// # Example
	/// ```rust
	/// use futures::join;
	/// use tokio::{io, task};
	/// use tokio::io::{AsyncReadExt, AsyncWriteExt};
	/// use tokio::process::Command;
	/// use tokio::task::spawn_local;
	/// use pedestal_rs::tokio_proc::ChildWrapper;
	/// let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
	///     let command = Command::new("cat");
	///     let mut child = ChildWrapper::spawn(command, ".").await?;
	///
	///     let mut stdin = child.stdin.take().unwrap();
	///     let mut stdout = child.stdout.take().unwrap();
	///     let local = task::LocalSet::new();
	///
	///     local.run_until(async move {
	///         let write_fut = spawn_local(async move {
	///             stdin.write_all(b"Hello\n").await?;
	///             stdin.flush().await?;
	///             stdin.shutdown().await?;
	///             io::Result::<()>::Ok(())
	///         });
	///         let read_fut = spawn_local(async move {
	///             let mut buf = vec![0; 16];
	///             let len = stdout.read(&mut buf).await?;
	///             assert_eq!(len, 6);
	///             assert_eq!(&buf[..len], b"Hello\n");
	///             io::Result::<()>::Ok(())
	///         });
	///         let (l, r) = join!(write_fut,read_fut);
	///         l??;
	///         r??;
	///         let ex = child.kill().await?;
	///         assert!(ex.success());
	///         io::Result::<()>::Ok(())
	///     }).await
	/// });
	/// assert!(result.is_ok())
	/// ```
	pub async fn spawn(mut command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		let dir = dir.as_ref();
		let stdin_path = dir.join("stdin");
		let stdout_path = dir.join("stdout");
		let stderr_path = dir.join("stderr");

		if metadata(&stdin_path).await.is_ok() { remove_file(&stdin_path).await?; }
		if metadata(&stdout_path).await.is_ok() { remove_file(&stdout_path).await?; }
		if metadata(&stderr_path).await.is_ok() { remove_file(&stderr_path).await?; }

		mkfifo(&stdin_path, Mode::S_IRWXU).map_err(io::Error::from)?;
		mkfifo(&stdout_path, Mode::S_IRWXU).map_err(io::Error::from)?;
		mkfifo(&stderr_path, Mode::S_IRWXU).map_err(io::Error::from)?;

		let sin_path = stdin_path.clone();
		let sout_path = stdout_path.clone();
		let serr_path = stderr_path.clone();

		let handle = spawn(async move {
			let stdin_handler = tokio::fs::OpenOptions::new().write(true).open(sin_path).await?;
			let stdout_handler = tokio::fs::OpenOptions::new().read(true).open(sout_path).await?;
			let stderr_handler = tokio::fs::OpenOptions::new().read(true).open(serr_path).await?;

			io::Result::<(File, File, File)>::Ok((stdin_handler, stdout_handler, stderr_handler))
		});

		let stdin = OpenOptions::new().read(true).open(&stdin_path)?;
		let stdout = OpenOptions::new().write(true).open(&stdout_path)?;
		let stderr = OpenOptions::new().write(true).open(&stderr_path)?;

		let child = command.stdin(Stdio::from(stdin))
			.stdout(Stdio::from(stdout))
			.stderr(Stdio::from(stderr))
			.spawn()?;

		let (stdin_handler, stdout_handler, stderr_handler) = handle.await??;

		Ok(Self {
			handle: ChildHandle::Owned(child),
			stdin: Some(Box::new(stdin_handler)),
			stdout: Some(Box::new(stdout_handler)),
			stderr: Some(Box::new(stderr_handler)),
		})
	}

	/// Attach to existing process id, only if spawned via [`Self::spawn`] work_dir should be the same
	pub async fn attach(pid: i32, work_dir: impl AsRef<Path>) -> io::Result<Self> {
		let handle = ChildHandle::Attached(Pid::from_raw(pid));
		let dir = work_dir.as_ref();
		let stdin = File::create(dir.join("stdin")).await?;
		let stdout = File::open(dir.join("stdout")).await?;
		let stderr = File::open(dir.join("stderr")).await?;
		Ok(Self {
			handle,
			stdin: Some(Box::new(stdin)),
			stdout: Some(Box::new(stdout)),
			stderr: Some(Box::new(stderr)),
		})
	}

	/// Same as [`tokio::process::Child::id`]
	pub fn id(&self) -> Option<u32> {
		match &self.handle {
			ChildHandle::Owned(ch) => { ch.id() }
			ChildHandle::Attached(id) => { Some(id.as_raw() as _) }
		}
	}

	/// Same as [`tokio::process::Child::wait`]
	pub async fn wait(&mut self) -> io::Result<ExitStatus> {
		match &mut self.handle {
			ChildHandle::Owned(ch) => { ch.wait().await }
			ChildHandle::Attached(id) => {
				let id = *id;
				spawn_blocking(move || {
					match waitpid(id, None) {
						Ok(status) => {
							Ok(wait_to_exit(status))
						}
						Err(err) => {
							Err(std::io::Error::from(err))
						}
					}
				}).await.map_err(|err| io::Error::new(ErrorKind::Other, err))?
			}
		}
	}

	/// Same as [`tokio::process::Child::start_kill`]
	pub fn start_kill(&mut self) -> io::Result<()> {
		match &mut self.handle {
			ChildHandle::Owned(ch) => { ch.start_kill() }
			ChildHandle::Attached(pid) => { kill(*pid, Signal::SIGKILL).map_err(io::Error::from) }
		}
	}

	/// Same as [`tokio::process::Child::kill`]
	pub async fn kill(&mut self) -> io::Result<ExitStatus> {
		self.start_kill()?;
		self.wait().await
	}

	/// Same as [`tokio::process::Child::try_wait`]
	pub async fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
		match &mut self.handle {
			ChildHandle::Owned(ch) => { ch.try_wait() }
			ChildHandle::Attached(pid) => {
				let wait_res = waitpid(*pid, Some(WaitPidFlag::WNOHANG)).map_err(io::Error::from)?;
				if wait_res.pid().is_none() {
					return Ok(None);
				}
				Ok(Some(wait_to_exit(wait_res)))
			}
		}
	}
}

fn wait_to_exit(wait: WaitStatus) -> ExitStatus {
	match wait {
		WaitStatus::Exited(_, status) => {
			ExitStatus::from_raw(status)
		}
		WaitStatus::Signaled(_, sig, _)
		| WaitStatus::Stopped(_, sig)
		| WaitStatus::PtraceEvent(_, sig, _) => {
			ExitStatus::from_raw(sig as i32)
		}
		WaitStatus::PtraceSyscall(_) => {
			ExitStatus::from_raw(1)
		}
		WaitStatus::Continued(_) => {
			ExitStatus::from_raw(1)
		}
		WaitStatus::StillAlive => {
			ExitStatus::from_raw(1)
		}
	}
}