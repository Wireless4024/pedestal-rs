#[deny(missing_docs)]
/// helper related to file system
#[cfg(feature = "fs")]
pub mod fs;

/// helper related to collection (data structure with multiple elements)
#[cfg(feature = "collection")]
pub mod collection;

/// extension helper
#[cfg(feature = "mutation")]
pub mod ext;

/// Tokio process helper
#[cfg(feature = "tokio-proc")]
pub mod tokio_proc;

/// Mini bitmap builder
#[cfg(feature = "mini-bmp")]
pub mod mini_bmp;

/// Helper to serialize [opencv::core::Mat]
#[cfg(feature = "cv-mat")]
pub mod cv_mat;