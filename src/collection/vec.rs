/// Create new vec with given size and use result from `init()` as default value  
/// this macro existed because vec![] won't work on `!Clone` value
#[macro_export]
macro_rules! new_vec {
    ($init:expr; $size:expr) => {
	    {
		    let size = $size;
		    let mut data = Vec::with_capacity(size);
			for _ in 0..size { data.push($init); }
			data
	    }
    };
}

#[cfg(test)]
mod test {
	#[test]
	fn new_vec_test() {
		let data = new_vec!(Option::<usize>::None; 4);
		assert_eq!(data, vec![None, None, None, None])
	}
}