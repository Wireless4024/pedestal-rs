use std::fs::write;

use opencv::core::Vector;
use opencv::imgcodecs::imwrite;

use pedestal_rs::cv_mat::CvMat;
use pedestal_rs::mini_bmp::BitMap;

fn main() {
	let mut bmp = BitMap::new(4, 4);
	for x in bmp.pixels_mut() {
		x.r = 127;
		x.a = 255;
	}
	write("test.bmp", &*bmp).unwrap();
	let mat = CvMat::from(&bmp);
	imwrite("test.png", &mat.to_mat().unwrap(), &Vector::new()).unwrap();
}