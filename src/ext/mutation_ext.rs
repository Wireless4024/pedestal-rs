use std::sync::Arc;

pub trait CloneExt<T> {
	/// Clone current variable and accept closure to modify its value
	/// # Example
	/// ```rust
	/// use pedestal_rs::ext::CloneExt;
	/// let base = "Hello".to_string();
	/// let copied = base.as_owned(|it| it.push_str(" world"));
	/// assert_eq!(&copied, "Hello world");
	/// ```
	#[must_use]
	fn as_owned<F: FnOnce(&mut T)>(&self, f: F) -> T;
}

pub trait ArcExt<T> {
	/// Create copy of this `Arc<T>`  
	/// this method accept closure to modify its value before creating new `Arc<T>`
	/// # Example
	/// ```rust
	/// use std::sync::Arc;
	/// use pedestal_rs::ext::ArcExt;
	/// let base = Arc::new("Hello".to_string());
	/// let copied = base.as_owned_arc(|it| it.push_str(" world"));
	/// assert_eq!(copied, Arc::new("Hello world".to_string()));
	/// ```
	#[must_use]
	fn as_owned_arc<F: FnOnce(&mut T)>(&self, f: F) -> Arc<T>;
	
	/// Modify Arc's content by replace old value with new modified value  
	/// this method will return old `Arc<T>`
	/// # Example
	/// ```rust
	/// use std::sync::Arc;
	/// use pedestal_rs::ext::ArcExt;
	/// let mut base = Arc::new("Hello".to_string());
	/// // create another strong reference
	/// let _copied1 = Arc::clone(&base);
 	/// base.modify(|it| it.clear());
	/// assert_eq!(base, Arc::new(String::new()));
	/// ```
	fn modify<F: FnOnce(&mut T)>(&mut self, f: F) -> Arc<T>;
}

impl<T: Clone> CloneExt<T> for T {
	#[inline]
	fn as_owned<F: FnOnce(&mut T)>(&self, f: F) -> T {
		let mut tmp = T::clone(self);
		f(&mut tmp);
		tmp
	}
}

impl<T: Clone> ArcExt<T> for Arc<T> {
	#[inline]
	fn as_owned_arc<F: FnOnce(&mut T)>(&self, f: F) -> Arc<T> {
		let mut tmp = T::clone(self);
		f(&mut tmp);
		Arc::new(tmp)
	}

	#[inline]
	fn modify<F: FnOnce(&mut T)>(&mut self, f: F) -> Arc<T> {
		let old = Arc::clone(self);
		*self = self.as_owned_arc(f);
		old
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use crate::ext::{ArcExt, CloneExt};

	#[test]
	pub fn test_clone() {
		let base = "Hello".to_string();
		let copied = base.as_owned(|it| it.push_str(" world"));
		assert_eq!(&copied, "Hello world");
	}

	#[test]
	pub fn test_clone_arc() {
		let base = Arc::new("Hello".to_string());
		let mut copied = base.as_owned_arc(|it| it.push_str(" world"));
		assert_eq!(copied, Arc::new("Hello world".to_string()));
		// has another strong reference
		let _copied1 = Arc::clone(&copied);
		copied.modify(|it| it.clear());
		assert_eq!(copied, Arc::new(String::new()));
	}
}