use std::mem;

use crate::new_vec;

/// CircularVec is used to store continuous stream of data and discard oldest element when full  
/// ### Usage
/// + store interval of data eg. load average every second size=15 will store latest 15 seconds
/// + store lines output from another process
pub struct CircularVec<T> {
	vec: Vec<Option<T>>,
	head: usize,
	tail: usize,
}

impl<T> CircularVec<T> {
	/// Create new circular vec with given size
	pub fn new(size: usize) -> Self {
		Self {
			vec: new_vec!(None; size),
			head: 0,
			tail: 0,
		}
	}

	#[inline]
	fn advance_head(&mut self) {
		let cap = self.vec.len();
		let head = self.head;
		if head + 1 == cap {
			self.head = 0;
		} else {
			self.head += 1;
		}
	}

	#[inline]
	fn advance_tail(&mut self) {
		let cap = self.vec.len();
		let tail = self.tail;
		if tail + 1 == cap {
			self.tail = 0;
		} else {
			self.tail += 1;
		}
	}

	/// Append value to ends of vec; if vec is full it will return oldest element
	pub fn push(&mut self, item: T) -> Option<T> {
		let head = self.head;
		let last = mem::replace(&mut self.vec[head], Some(item));

		// if head == tail can be empty or full  
		// if last element is present vec is full otherwise empty
		if self.head == self.tail && last.is_some() {
			self.advance_tail();
		}
		self.advance_head();
		last
	}

	/// Try to remove oldest element from vec
	pub fn pop(&mut self) -> Option<T> {
		let head = self.head;
		let tail = self.tail;
		if head == tail {
			let ent = mem::take(&mut self.vec[tail]);
			if ent.is_some() {
				self.advance_tail();
			}
			return ent;
		}
		self.advance_tail();
		mem::take(&mut self.vec[tail])
	}

	/// Get length of this circular vec
	pub fn len(&self) -> usize {
		let head = self.head;
		let tail = self.tail;
		if tail < head {
			head - tail
		} else {
			// head == tail; so this vec is full
			self.vec.len()
		}
	}

	/// Check if this circular vec is empty
	pub fn is_empty(&self) -> bool {
		self.vec[self.tail].is_none()
	}

	/// Check if this circular vec is full
	pub fn is_full(&self) -> bool {
		let head = self.head;
		let tail = self.tail;
		head == tail && self.vec[tail].is_some()
	}

	/// Take all data from this circular into vec
	pub fn take(&mut self) -> Vec<T> {
		let items = self.len();
		if items == 0 {
			return Vec::new();
		}
		let mut out_vec = Vec::with_capacity(items);
		while let Some(elem) = self.pop() { out_vec.push(elem); }
		out_vec
	}

	pub fn iter(&self) -> CircularVecIter<T> {
		CircularVecIter(self, 0)
	}
}

pub struct CircularVecIter<'a, T>(&'a CircularVec<T>, usize);

impl<'a, T> Iterator for CircularVecIter<'a, T> {
	type Item = &'a T;

	fn next(&mut self) -> Option<Self::Item> {
		let len = self.0.len();
		if self.1 >= len {
			None
		} else {
			let idx = self.1;
			self.1 += 1;
			self.0.vec[(self.0.tail + idx) % self.0.vec.len()].as_ref()
		}
	}
}

#[cfg(test)]
mod test {
	use crate::collection::CircularVec;

	#[test]
	fn test_circular_vec() {
		let mut vec = CircularVec::new(4);
		vec.push(1);
		assert_eq!(Some(1), vec.pop());
		vec.push(2);
		vec.push(3);
		assert_eq!(Some(2), vec.pop());
		assert_eq!(Some(3), vec.pop());
		vec.push(4);
		vec.push(5);
		vec.push(6);
		assert_eq!(vec![&4, &5, &6], vec.iter().collect::<Vec<&i32>>());
		assert_eq!(Some(4), vec.pop());
		assert_eq!(Some(5), vec.pop());
		assert_eq!(Some(6), vec.pop());
		assert_eq!(None, vec.pop());
	}
}