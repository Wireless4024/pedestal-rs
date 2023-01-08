use std::alloc::{alloc, Layout};
use std::io::{Read, Write};
use std::mem;
use std::mem::{ManuallyDrop, size_of};

use opencv::core::{Mat, MatTraitConst};
use opencv::prelude::MatTrait;

#[derive(Debug)]
#[repr(C)]
pub struct MatHeader {
	/// version (unused)
	pub ver: u8,
	// 8
	_reserved1: u8,
	// 16
	_reserved2: u16,
	// 32
	_reserved3: u32,
	// 64
	/// Matrix width
	pub width: i32,
	// 96
	/// Matrix height
	pub height: i32,
	// 128
	/// Matrix type
	pub mat_format: i32,
	_reserved4: u32,
	_reserved5: u64,
}

/// [opencv::core::Mat] Serializer / Deserializer
pub struct CvMat {
	/// header contains information of [opencv::core::Mat]
	pub header: MatHeader,
	/// raw data in [opencv::core::Mat]
	pub data: Vec<u8>,
}

const MAT_VER: u8 = 1;

impl From<&Mat> for MatHeader {
	fn from(value: &Mat) -> Self {
		Self {
			ver: MAT_VER,
			_reserved1: 0,
			_reserved2: 0,
			_reserved3: 0,
			width: value.cols(),
			height: value.rows(),
			mat_format: value.typ(),
			_reserved4: 0,
			_reserved5: 0,
		}
	}
}

impl From<&Mat> for CvMat {
	fn from(value: &Mat) -> Self {
		let header = MatHeader::from(value);

		let mut data = header.alloc_vec();
		if value.is_continuous() {
			unsafe { data.as_mut_ptr().copy_from(value.datastart(), data.len()) };
		} else {
			let width = (header.width as usize) * (header.channels() as usize) * (header.depth_width() as usize);
			for i in 0..header.height {
				let row_ptr = unsafe { data.as_mut_ptr().add(width * (i as usize)) };
				if let Ok(row) = value.row(i) {
					use opencv::prelude::MatTraitConstManual;
					unsafe { row_ptr.copy_from(row.data(), width) };
				};
			}
		}
		Self {
			header,
			data,
		}
	}
}

impl CvMat {
	pub fn read_to_mat<R: Read>(r: &mut R) -> std::io::Result<Mat> {
		let mut head = [0u8; 32];
		r.read_exact(&mut head)?;
		let header = unsafe { std::mem::transmute::<_, MatHeader>(head) };
		let mut mat = header.alloc_mat().map_err(|it| std::io::Error::new(std::io::ErrorKind::Other, it))?;
		let mut raw = unsafe {
			ManuallyDrop::new(Vec::from_raw_parts(mat.data_mut(), header.data_len(), header.data_len()))
		};
		r.read_exact(&mut raw)?;
		Ok(mat)
	}

	pub fn read<R: Read>(r: &mut R) -> std::io::Result<Self> {
		let mut head = [0u8; 32];
		r.read_exact(&mut head)?;
		let header = unsafe { std::mem::transmute::<_, MatHeader>(head) };
		let mut data = header.alloc_vec();
		r.read_exact(&mut data)?;
		Ok(Self {
			header,
			data,
		})
	}

	pub fn to_mat(&self) -> opencv::Result<Mat> {
		let MatHeader { width, height, mat_format, .. } = self.header;
		let mut mat = self.header.alloc_mat()?;
		unsafe { mat.data_mut().copy_from(self.data.as_ptr(), self.data.len()); }
		Ok(mat)
	}

	pub fn write<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
		assert_eq!(size_of::<[u8; 32]>(), size_of::<MatHeader>());
		let header = unsafe { mem::transmute::<_, &[u8; 32]>(&self.header) };
		w.write_all(header)?;
		w.write_all(&self.data)
	}
}

impl MatHeader {
	pub fn alloc_mat(&self) -> opencv::Result<Mat> {
		unsafe { Mat::new_rows_cols(self.height, self.width, self.mat_format) }
	}

	#[inline]
	pub fn channels(&self) -> u8 {
		let ch = ((self.mat_format >> 3) as u8) & 7;
		ch + 1
	}

	#[inline]
	pub fn depth(&self) -> i32 {
		self.mat_format & 7
	}

	#[inline]
	pub fn depth_width(&self) -> i32 {
		match self.depth() {
			opencv::core::CV_8U | opencv::core::CV_8S => {
				1
			}
			opencv::core::CV_16U | opencv::core::CV_16S => {
				2
			}
			opencv::core::CV_32F | opencv::core::CV_32S => {
				4
			}
			opencv::core::CV_64F => {
				8
			}
			_ => 1
		}
	}

	fn data_len(&self) -> usize {
		(self.width as usize)
			* (self.height as usize)
			* (self.depth_width() as usize)
			* (self.channels() as usize)
	}

	fn alloc_vec(&self) -> Vec<u8> {
		let size = self.data_len();
		unsafe {
			let layout = Layout::array::<u8>(size).unwrap();
			let ptr = alloc(layout);
			Vec::from_raw_parts(ptr, size, layout.size())
		}
	}
}