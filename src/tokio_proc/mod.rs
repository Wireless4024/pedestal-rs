use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::AtomicBool;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};

pub struct ChildWrapper {
	handle: ChildHandle,
	pub stdin: Option<Box<dyn AsyncWrite + Unpin>>,
	pub stdout: Option<Box<dyn AsyncRead + Unpin>>,
	pub stderr: Option<Box<dyn AsyncRead + Unpin>>,
}

enum ChildHandle {
	Owned(Child),
	#[cfg(target_os = "linux")]
	Attached(nix::unistd::Pid),
}

#[cfg(target_os = "linux")]
static FORK_ENV: &str = "TK_PROC_FORK_HANDLE";
#[cfg(target_os = "linux")]
static FORK_REQ_ENV: &str = "TK_PROC_FORK_HANDLE_PLS";
#[cfg(target_os = "linux")]
static PID_FILE_PATH: &str = ".pid";
#[cfg(target_os = "linux")]
static GUARD_FILE_PATH: &str = ".gpid";
#[cfg(target_os = "linux")]
static STDIN_FILE_PATH: &str = "stdin";
#[cfg(target_os = "linux")]
static STDOUT_FILE_PATH: &str = "stdout";
#[cfg(target_os = "linux")]
static STDERR_FILE_PATH: &str = "stderr";

impl ChildWrapper {
	/// Spawn command and make it re-attachable;  
	/// but can't re-attach after parent was exit,  
	/// use [`ProcessHandle::hold_process`] to keep child running
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
	#[cfg(target_os = "linux")]
	pub async fn spawn(mut command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		use tokio::fs::{File, metadata, remove_file};
		let dir = dir.as_ref();
		let stdin_path = dir.join(STDIN_FILE_PATH);
		let stdout_path = dir.join(STDOUT_FILE_PATH);
		let stderr_path = dir.join(STDERR_FILE_PATH);

		if metadata(&stdin_path).await.is_ok() { remove_file(&stdin_path).await?; }
		if metadata(&stdout_path).await.is_ok() { remove_file(&stdout_path).await?; }
		if metadata(&stderr_path).await.is_ok() { remove_file(&stderr_path).await?; }

		use nix::unistd::mkfifo;
		mkfifo(&stdin_path, nix::sys::stat::Mode::S_IRWXU).map_err(io::Error::from)?;
		mkfifo(&stdout_path, nix::sys::stat::Mode::S_IRWXU).map_err(io::Error::from)?;
		mkfifo(&stderr_path, nix::sys::stat::Mode::S_IRWXU).map_err(io::Error::from)?;

		let sin_path = stdin_path.clone();
		let sout_path = stdout_path.clone();
		let serr_path = stderr_path.clone();

		let handle = tokio::spawn(async move {
			let stdin_handler = tokio::fs::OpenOptions::new().write(true).open(sin_path).await?;
			let stdout_handler = tokio::fs::OpenOptions::new().read(true).open(sout_path).await?;
			let stderr_handler = tokio::fs::OpenOptions::new().read(true).open(serr_path).await?;

			io::Result::<(File, File, File)>::Ok((stdin_handler, stdout_handler, stderr_handler))
		});

		use std::fs::OpenOptions;
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

	#[cfg(target_os = "linux")]
	pub async fn spawn_noerr(mut command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		use tokio::fs::{File, metadata, remove_file};
		let dir = dir.as_ref();
		let stdin_path = dir.join(STDIN_FILE_PATH);
		let stdout_path = dir.join(STDOUT_FILE_PATH);
		let stderr_path = dir.join(STDERR_FILE_PATH);

		if metadata(&stdin_path).await.is_ok() { remove_file(&stdin_path).await?; }
		if metadata(&stdout_path).await.is_ok() { remove_file(&stdout_path).await?; }
		if metadata(&stderr_path).await.is_ok() { remove_file(&stderr_path).await?; }

		use nix::unistd::mkfifo;
		mkfifo(&stdin_path, nix::sys::stat::Mode::S_IRWXU).map_err(io::Error::from)?;
		mkfifo(&stdout_path, nix::sys::stat::Mode::S_IRWXU).map_err(io::Error::from)?;

		let sin_path = stdin_path.clone();
		let sout_path = stdout_path.clone();

		let handle = tokio::spawn(async move {
			let stdin_handler = tokio::fs::OpenOptions::new().write(true).open(sin_path).await?;
			let stdout_handler = tokio::fs::OpenOptions::new().read(true).open(sout_path).await?;

			io::Result::<(File, File)>::Ok((stdin_handler, stdout_handler))
		});

		use std::fs::OpenOptions;
		let stdin = OpenOptions::new().read(true).open(&stdin_path)?;
		let stdout = OpenOptions::new().write(true).open(&stdout_path)?;

		let child = command.stdin(Stdio::from(stdin))
			.stdout(Stdio::from(stdout))
			.stderr(Stdio::null())
			.spawn()?;

		let (stdin_handler, stdout_handler) = handle.await??;

		Ok(Self {
			handle: ChildHandle::Owned(child),
			stdin: Some(Box::new(stdin_handler)),
			stdout: Some(Box::new(stdout_handler)),
			stderr: None,
		})
	}

	#[cfg(not(target_os = "linux"))]
	pub async fn spawn_noerr(command: Command, _dir: impl AsRef<Path>) -> io::Result<Self> {
		Self::spawn(command, _dir).await
	}

	/// re-attach didn't implemented on this platform (you will see this if not running in linux)  
	/// scroll-up if you want to see docs
	#[cfg(not(target_os = "linux"))]
	pub async fn spawn(mut command: Command, _dir: impl AsRef<Path>) -> io::Result<Self> {
		let mut child = command.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()?;
		let stdin = child.stdin.take().map(|it| {
			let r: Box<dyn AsyncWrite + Unpin> = Box::new(it);
			r
		});
		let stdout = child.stdout.take().map(|it| {
			let r: Box<dyn AsyncRead + Unpin> = Box::new(it);
			r
		});
		let stderr = child.stderr.take().map(|it| {
			let r: Box<dyn AsyncRead + Unpin> = Box::new(it);
			r
		});

		Ok(Self {
			handle: ChildHandle::Owned(child),
			stdin,
			stdout,
			stderr,
		})
	}

	#[cfg(not(target_os = "linux"))]
	pub async fn spawn_hold(command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		Self::spawn(command, dir.as_ref()).await
	}

	#[cfg(not(target_os = "linux"))]
	pub async fn spawn_hold_noerr(command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		Self::spawn_noerr(command, dir.as_ref()).await
	}

	#[cfg(target_os = "linux")]
	pub async fn spawn_hold(command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		let out = Self::spawn(command, dir.as_ref()).await?;
		ProcessHandle::hold_process(dir.as_ref().to_string_lossy().to_string()).await?;
		Ok(out)
	}

	#[cfg(target_os = "linux")]
	pub async fn spawn_hold_noerr(command: Command, dir: impl AsRef<Path>) -> io::Result<Self> {
		let out = Self::spawn_noerr(command, dir.as_ref()).await?;
		ProcessHandle::hold_process(dir.as_ref().to_string_lossy().to_string()).await?;
		Ok(out)
	}

	/// Attach to existing process id, only if spawned via [`Self::spawn`] work_dir should be the same
	/// # Return
	/// + Ok(None) if can't attach or child can't be found
	/// + Ok(Some(ChildWrapper)) if attached 
	pub async fn attach(_pid: i32, _work_dir: impl AsRef<Path>) -> io::Result<Option<Self>> {
		#[cfg(not(target_os = "linux"))]
		{
			return Ok(None);
		}
		#[cfg(target_os = "linux")]
		{
			use nix::sys::wait::waitpid;
			use nix::sys::wait::WaitPidFlag;
			use nix::unistd::Pid;
			use tokio::fs::File;

			if waitpid(Pid::from_raw(_pid), Some(WaitPidFlag::WNOHANG)).is_err() {
				return Ok(None);
			}
			let handle = ChildHandle::Attached(Pid::from_raw(_pid));
			let dir = _work_dir.as_ref();
			let stdin = File::create(dir.join(STDIN_FILE_PATH)).await?;
			let stdout = File::open(dir.join(STDOUT_FILE_PATH)).await?;
			let stderr = File::open(dir.join(STDERR_FILE_PATH)).await?;
			Ok(Some(Self {
				handle,
				stdin: Some(Box::new(stdin)),
				stdout: Some(Box::new(stdout)),
				stderr: Some(Box::new(stderr)),
			}))
		}
	}

	/// Same as [`Child::id`]
	pub fn id(&self) -> Option<u32> {
		match &self.handle {
			ChildHandle::Owned(ch) => { ch.id() }
			#[cfg(target_os = "linux")]
			ChildHandle::Attached(id) => { Some(id.as_raw() as _) }
		}
	}

	/// Same as [`Child::wait`]
	pub async fn wait(&mut self) -> io::Result<ExitStatus> {
		match &mut self.handle {
			ChildHandle::Owned(ch) => { ch.wait().await }
			#[cfg(target_os = "linux")]
			ChildHandle::Attached(id) => {
				let id = *id;
				tokio::task::spawn_blocking(move || {
					use nix::sys::wait::waitpid;
					match waitpid(id, None) {
						Ok(status) => {
							Ok(wait_to_exit(status))
						}
						Err(err) => {
							Err(std::io::Error::from(err))
						}
					}
				}).await.map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
			}
		}
	}

	/// Same as [`Child::start_kill`]
	pub fn start_kill(&mut self) -> io::Result<()> {
		match &mut self.handle {
			ChildHandle::Owned(ch) => { ch.start_kill() }
			#[cfg(target_os = "linux")]
			ChildHandle::Attached(pid) => {
				use nix::sys::signal::Signal;
				nix::sys::signal::kill(*pid, Signal::SIGKILL).map_err(io::Error::from)
			}
		}
	}

	/// Same as [`Child::kill`]
	pub async fn kill(&mut self) -> io::Result<ExitStatus> {
		self.start_kill()?;
		self.wait().await
	}

	/// Same as [`Child::try_wait`]
	pub async fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
		match &mut self.handle {
			ChildHandle::Owned(ch) => { ch.try_wait() }
			#[cfg(target_os = "linux")]
			ChildHandle::Attached(pid) => {
				use nix::sys::wait::waitpid;
				use nix::sys::wait::WaitPidFlag;
				let wait_res = waitpid(*pid, Some(WaitPidFlag::WNOHANG)).map_err(io::Error::from)?;
				if wait_res.pid().is_none() {
					return Ok(None);
				}
				Ok(Some(wait_to_exit(wait_res)))
			}
		}
	}
}

#[cfg(target_os = "linux")]
fn wait_to_exit(wait: nix::sys::wait::WaitStatus) -> ExitStatus {
	use nix::sys::wait::WaitStatus;
	use std::os::unix::prelude::ExitStatusExt;
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

#[cfg(target_os = "linux")]
fn get_val<T: std::str::FromStr>(path: impl AsRef<Path>) -> Option<T> {
	let mut file = std::fs::File::open(path).ok()?;
	let mut content = String::new();
	use std::io::Read;
	file.read_to_string(&mut content).ok()?;
	content.parse::<T>().ok()
}

#[cfg(target_os = "linux")]
fn put_val(path: impl AsRef<Path>, content: impl std::fmt::Display) -> io::Result<()> {
	let mut file = std::fs::File::create(path)?;
	use std::io::Write;
	file.write_all(content.to_string().as_bytes())?;
	Ok(())
}

pub struct ProcessHandle {
	#[cfg(target_os = "linux")]
	path: PathBuf,
}

#[allow(unused)]
static HAS_HOOK: AtomicBool = AtomicBool::new(false);

impl ProcessHandle {
	pub fn new(_path: impl Into<PathBuf>) -> Self {
		Self {
			#[cfg(target_os = "linux")]
			path: _path.into()
		}
	}

	#[inline]
	#[cfg(target_os = "linux")]
	fn run(self) -> io::Result<()> {
		use std::fs::File;
		use nix::sys::wait::waitpid;
		use nix::sys::wait::WaitPidFlag;
		use nix::unistd::Pid;

		let dir = &self.path;

		let pid = get_val::<i32>(dir.join(PID_FILE_PATH))
			.map(Pid::from_raw)
			.ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;

		let exit = waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(std::io::Error::from)?;
		if exit.pid().is_some() {
			eprintln!("Process already exit");
			return Ok(());
		}
		let _stdin = File::create(dir.join(STDIN_FILE_PATH))?;
		let _stdout = File::open(dir.join(STDOUT_FILE_PATH))?;
		let _stderr = File::open(dir.join(STDERR_FILE_PATH)).ok();

		loop {
			let status = waitpid(pid, Some(WaitPidFlag::WNOHANG)).map_err(std::io::Error::from)?;
			if status.pid().is_some() {
				break;
			}
			std::thread::sleep(std::time::Duration::from_secs(1));
		}

		Ok(())
	}

	#[cfg(target_os = "linux")]
	unsafe fn fork(path: String) {
		use nix::unistd::fork;
		match fork().expect("Can't fork") {
			nix::unistd::ForkResult::Parent { child } => {
				use nix::sys::wait::waitpid;
				waitpid(Some(child), None).unwrap();
			}
			nix::unistd::ForkResult::Child => {
				nix::unistd::setsid().expect("Create new session");
				use std::env::args_os;
				let this = args_os().next().unwrap();
				let guard = Command::new(this)
					.stdin(Stdio::null())
					.stdout(Stdio::null())
					.stderr(Stdio::null())
					.env(FORK_ENV, &path)
					.spawn()
					.unwrap();
				let pid = guard.id().unwrap() as i32;
				put_val(Path::join(path.as_ref(), GUARD_FILE_PATH), pid).ok();
			}
		}
		std::process::exit(0);
	}


	#[cfg(target_os = "linux")]
	async fn hold_process(path: impl AsRef<std::ffi::OsStr>) -> io::Result<ExitStatus> {
		use std::sync::atomic::Ordering;
		if !HAS_HOOK.load(Ordering::Relaxed) {
			return Err(io::Error::new(io::ErrorKind::Unsupported, "put `ProcessHandle::fork_hook` at earliest call in `main()` to use this feature"));
		}
		use std::env::args_os;
		let this = args_os().next().unwrap();
		let mut handle = Command::new(this)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.env(FORK_REQ_ENV, path)
			.spawn()
			.unwrap();
		handle.wait().await
	}

	/// Does nothing if not running on linux
	#[cfg(not(target_os = "linux"))]
	pub fn fork_hook() {}

	/// This hook should come before anything even async runtime initialization  
	/// due fork() may yield unexpected behavior 
	#[cfg(target_os = "linux")]
	pub fn fork_hook() {
		use std::sync::atomic::Ordering;
		HAS_HOOK.store(true, Ordering::Relaxed);
		if let Ok(path) = std::env::var(FORK_REQ_ENV) {
			unsafe { Self::fork(path); }
		}
		if let Ok(path) = std::env::var(FORK_ENV) {
			Self::new(path).run().ok();
		}
	}
}