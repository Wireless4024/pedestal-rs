use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use futures::executor::block_on;

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

pub trait ArcExt<T: 'static> {
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

	/// # Example
	/// ```rust
	/// use std::sync::Arc;
	/// use futures::executor::block_on;
	/// use pedestal_rs::ext::ArcExt;
	/// let mut base = Arc::new("Hello".to_string());
	/// // has another strong reference
	/// let _copied1 = Arc::clone(&base);
	/// {
	///     let fut = base.modify_async(|it| Box::pin(async {
	///         it.clear();
	///     }));
	///     // you should replace `executor::block_on` with `.await`
	///     let res = block_on(fut);
	///     assert_eq!(_copied1, res);
	/// }
	/// assert_eq!(base, Arc::new(String::new()));
	/// assert_ne!(_copied1, Arc::new(String::new()));
	/// ```
	#[cfg(feature = "async")]
	fn modify_async<'a, F>(&'a mut self, f: F) -> Pin<Box<dyn Future<Output=Arc<T>> + 'a>>
		where for<'b> F: FnOnce(&'b mut T) -> Pin<Box<dyn Future<Output=()> + 'b>>,
		      F: 'a;

	/// This implementation block on &mut reference to make it Send+Sync
	/// # Example
	/// ```rust
	/// use std::sync::Arc;
	/// use futures::executor::block_on;
	/// use pedestal_rs::ext::ArcExt;
	/// let mut base = Arc::new("Hello".to_string());
	/// // has another strong reference
	/// let _copied1 = Arc::clone(&base);
	/// {
	///     base.modify_async_send(|it| Box::pin(async {
	///         it.clear();
	///     }));
	/// }
	/// assert_eq!(base, Arc::new(String::new()));
	/// assert_ne!(_copied1, Arc::new(String::new()));
	/// ```
	#[cfg(feature = "async")]
	fn modify_async_send<'a, F>(&'a mut self, f: F)
		where for<'b> F: FnOnce(&'b mut T) -> Pin<Box<dyn Future<Output=()> + Send + Sync + 'b>>,
		      F: 'a,;
}

impl<T: Clone> CloneExt<T> for T {
	#[inline]
	fn as_owned<F: FnOnce(&mut T)>(&self, f: F) -> T {
		let mut tmp = T::clone(self);
		f(&mut tmp);
		tmp
	}
}

impl<T: Clone + 'static> ArcExt<T> for Arc<T> {
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

	#[cfg(feature = "async")]
	fn modify_async<'a, F>(&'a mut self, f: F) -> Pin<Box<dyn Future<Output=Arc<T>> + 'a>>
		where for<'b> F: FnOnce(&'b mut T) -> Pin<Box<dyn Future<Output=()> + 'b>>,
		      F: 'a {
		let old = Arc::clone(self);
		let mut new = T::clone(self);
		Box::pin(async {
			f(&mut new).await;
			*self = Arc::new(new);
			old
		})
	}

	fn modify_async_send<'a, F>(&'a mut self, f: F)
		where
				for<'b> F: FnOnce(&'b mut T) -> Pin<Box<dyn Future<Output=()> + Send + Sync + 'b>>,
				F: 'a {
		let mut new = T::clone(self);
		block_on(f(&mut new));
		*self = Arc::new(new);
	}
}