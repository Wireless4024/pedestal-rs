use std::fs::File;

use opencv::core::Vector;
use opencv::imgcodecs::{imread, IMREAD_UNCHANGED, imwrite};

use pedestal_rs::cv_mat::CvMat;

fn main() {
	let mat = imread("image.png", IMREAD_UNCHANGED).expect("Read image");
	println!("create cvmat");
	let im = CvMat::from(&mat);
	let mut out = Vec::new();
	println!("write cvmat");
	im.write(&mut out).expect("Serialize");
	std::fs::write("image.mat", out).expect("Write file");
	println!("read cvmat");
	let mut r = File::open("image.mat").expect("Read mat");
	let cvmat = CvMat::read_to_mat(&mut r).expect("Read mat");
	imwrite("image2.png", &cvmat, &Vector::new()).expect("Write image");
}
