use std::alloc::Layout;
use std::ops::Deref;
use std::slice;

#[repr(C, align(2))]
pub struct BGRA {
	pub b: u8,
	pub g: u8,
	pub r: u8,
	pub a: u8,
}

/// This struct used to store bitmap data in memory
/// # Limitation
/// + RGBA 32bpp format only
/// + Can't read from anything that is not RGBA 32bpp
pub struct BitMap {
	data: Vec<u8>,
}

macro_rules! raw_access {
    ($r_name:ident, $w_name:ident, $bytes:literal, $var:ident) => {
		#[allow(unused)]
		fn $r_name(&self, offset: usize) -> Option<$var> {
			if offset >= self.data.len() { return None; }
			let mut buf = [0; $bytes];
			buf.copy_from_slice(&self.data[offset..offset + $bytes]);
			Some($var::from_le_bytes(buf))
		}
	
		#[allow(unused)]
		fn $w_name(&mut self, offset: usize, value: $var) {
			if offset >= self.data.len() { return; }
			let buf: [u8; $bytes] = value.to_le_bytes();
			self.data[offset..offset + $bytes].copy_from_slice(&buf);
		}
    };
}

static BMP_HEADER: &[u8] = &[b'B', b'M'];
/// u32
const BM_OFFSET: usize = 2;
/// u32; off + 4 reserve bytes
const BM_OFFSET_PIXEL_DATA: usize = BM_OFFSET + 8 /* 10 */;
/// u32
const BM_HEADER_SIZE: usize = BM_OFFSET_PIXEL_DATA + 4 /* 14 */;
/// i32
const BM_WIDTH: usize = BM_HEADER_SIZE + 4 /* 18 */;
/// i32
const BM_HEIGHT: usize = BM_WIDTH + 4 /* 22 */;
/// u16
const BM_PLANES: usize = BM_HEIGHT + 4 /* 26 */;
/// u16
const BM_BPP: usize = BM_PLANES + 2 /* 28 */;
/// u32
const BM_COMPRESSION: usize = BM_BPP + 2 /* 30 */;
/// u32
const BM_IMAGE_SIZE: usize = BM_COMPRESSION + 4 /* 34 */;
/// u32
const BM_X_PPM: usize = BM_IMAGE_SIZE + 4 /* 38 */;
/// u32
const BM_Y_PPM: usize = BM_X_PPM + 4 /* 42 */;
// u32
//const BM_TOTAL_COLORS: usize = BM_Y_PPM + 4 /* 46 */;
// u32
//const BM_IMPORTANT_COLOR: usize = BM_TOTAL_COLORS + 4 /* 50 */;

const BM_PIXEL_START: usize = 54;

macro_rules! check {
    ($cond:expr) => {
	    if !($cond) { return false }
    };
}

fn size_of<T>(_: T) -> usize { std::mem::size_of::<T>() }

macro_rules! check_by {
    ($data:ident[$off:ident], $var:expr) => {
	    {
		    let val = $var;
			let sz = size_of(val);
			if &$data[$off..$off+sz] != &val.to_le_bytes() { 
				return false;
			}
	    }
    };
}


impl BitMap {
	/// # Safety
	/// if input vec is output of this [BitMap::deref], this function is safe
	pub unsafe fn from_vec(data: Vec<u8>) -> Self {
		Self { data }
	}


	/// Create new bitmap from bytes slice (may return None if bitmap data is not compatible)
	pub fn from_raw(data: &[u8]) -> Option<Self> {
		if !Self::validate_compatible_header(data) {
			return None;
		}
		let layout = Layout::from_size_align(data.len(), 2).unwrap();
		let _data = unsafe {
			let ptr = std::alloc::alloc(layout);
			ptr.copy_from(data.as_ptr(), data.len());
			Vec::from_raw_parts(ptr, data.len(), layout.size())
		};
		Some(Self {
			data: _data,
		})
	}

	fn validate_compatible_header(data: &[u8]) -> bool {
		check!(&data[..BM_OFFSET] == b"BM");

		check_by!(data[BM_OFFSET_PIXEL_DATA], BM_PIXEL_START as u32);
		check_by!(data[BM_BPP], 32u32);
		check_by!(data[BM_COMPRESSION], 0u16);
		true
	}

	/// create new bitmap with given dimension filled with `#00000000` BGRA color
	pub fn new(width: i32, height: i32) -> BitMap {
		assert!(width > 0);
		assert!(height > 0);
		let len = 54 + (((width * height) as usize) << 2);
		let layout = Layout::from_size_align(len, 2).unwrap();
		let data = unsafe {
			let ptr = std::alloc::alloc_zeroed(layout);
			Vec::from_raw_parts(ptr, len, layout.size())
		};
		let mut it = Self { data };

		// copy header
		it.data[..2].copy_from_slice(BMP_HEADER);
		it.write_32(BM_OFFSET, it.data.len() as u32);
		it.write_32(BM_OFFSET_PIXEL_DATA, BM_PIXEL_START as u32);

		it.write_32(BM_HEADER_SIZE, 40u32);
		it.write_32(BM_WIDTH, width as _);
		it.write_32(BM_HEIGHT, height as _);
		it.write_16(BM_PLANES, 1);
		it.write_16(BM_BPP, 32);
		it.write_16(BM_X_PPM, 2835);
		it.write_16(BM_Y_PPM, 2835);
		it
	}

	raw_access! {read_16,write_16,2,u16}
	raw_access! {read_32,write_32,4,u32}

	/// Get width of this image
	pub fn width(&self) -> u32 {
		self.read_32(BM_WIDTH).unwrap()
	}

	/// Get height of this image
	pub fn height(&self) -> u32 {
		self.read_32(BM_WIDTH).unwrap()
	}

	/// Get pixel slice
	pub fn pixels(&self) -> &[BGRA] {
		unsafe {
			slice::from_raw_parts(self.pixel_bytes().as_ptr().cast::<BGRA>(), (self.width() * self.height()) as _)
		}
	}

	/// Get mutable pixel slice
	pub fn pixels_mut(&mut self) -> &mut [BGRA] {
		unsafe {
			slice::from_raw_parts_mut(self.data.as_mut_ptr().add(BM_PIXEL_START).cast::<BGRA>(), (self.width() * self.height()) as _)
		}
	}

	pub fn pixel_bytes(&self) -> &[u8] {
		let raw = &self.data;
		unsafe { slice::from_raw_parts(raw.as_ptr().add(BM_PIXEL_START).cast::<u8>(), (raw.len() - 54) * 4) }
	}

	/// Build pixel bytes for matrix image use
	pub fn build_mat(&self) -> Vec<u8> {
		let mut data = Vec::with_capacity(self.data.len() << 2);
		for x in self.pixel_bytes().windows((self.width() as usize) * 4).rev() {
			data.extend_from_slice(x);
		}
		data
	}
}

impl Deref for BitMap {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

#[cfg(test)]
mod test {
	use crate::mini_bmp::BitMap;

	#[test]
	fn test_bmp() {
		let bmp = BitMap::new(4, 4);
		std::fs::write("test.bmp", &*bmp).unwrap();
	}
}