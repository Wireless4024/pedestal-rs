use std::fs::canonicalize;
use std::io;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

///
/// Normalize malicious path input but keep it contains in base directory otherwise return `ErrorKind::InvalidInput`  
/// ref: https://github.com/rust-lang/rfcs/issues/2208#issuecomment-342679694
pub fn normalize(base: &Path, p: impl AsRef<Path>) -> io::Result<PathBuf> {
	let p = p.as_ref();
	let mut stack: Vec<Component> = Vec::new();
	for component in p.components() {
		match component {
			Component::CurDir => {}
			Component::ParentDir => {
				let top = stack.last();
				match top {
					Some(c) => {
						match c {
							Component::Prefix(_) => { stack.push(component); }
							Component::RootDir => {}
							Component::CurDir => { unreachable!(); }
							Component::ParentDir => { stack.push(component); }
							Component::Normal(_) => { let _ = stack.pop(); }
						}
					}
					None => { stack.push(component); }
				}
			}
			_ => { stack.push(component); }
		}
	}
	if stack.is_empty() { return Ok(PathBuf::from(base)); }
	let mut buf = PathBuf::with_capacity(1 + stack.len());
	buf.push(base);
	for x in stack {
		if let Component::ParentDir = x {
			return Err(ErrorKind::InvalidInput.into());
		}
		buf.push(x);
	}
	Ok(buf)
}

/// Get relative path to access source from target
pub fn relative_from(source: impl AsRef<Path>, target: impl AsRef<Path>) -> PathBuf {
	let source: &Path = source.as_ref();
	let mut target: &Path = target.as_ref();
	// if absolute; replace with relative location from root
	if target.is_absolute() {
		target = target.strip_prefix(canonicalize(".").unwrap()).unwrap();
	}
	let mut scomp = source.components().peekable();
	let mut tcomp = target.components().peekable();

	let mut relative = PathBuf::new();

	while let Some(comp) = scomp.peek() {
		if Some(comp) == tcomp.peek() {
			scomp.next();
			tcomp.next();
			continue;
		} else { break; }
	}

	for _ in tcomp { relative.push("../"); }
	for x in scomp { relative.push(x); }
	relative
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use crate::fs::path::{normalize, relative_from};

	#[test]
	fn test_normalize() {
		let base = PathBuf::from(".").canonicalize().unwrap();
		let mut want = PathBuf::from("ads/dsda/../../");
		let result = normalize(&base, &want);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), base);

		want.push("../");
		let result = normalize(&base, &want);
		assert!(result.is_err())
	}

	#[test]
	fn test_absolute_from1() {
		assert_eq!(relative_from("src/lib.rs", "target/debug"), PathBuf::from("../../src/lib.rs"));
		assert_eq!(relative_from("src/lib.rs", "src/fs"), PathBuf::from("../lib.rs"));
		assert_eq!(relative_from("../src/lib.rs", "target/debug"), PathBuf::from("../../../src/lib.rs"));
	}
}