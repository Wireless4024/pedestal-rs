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

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use crate::fs::path::normalize;

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
}