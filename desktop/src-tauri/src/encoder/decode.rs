use super::{EncodeError, ImageExt};
use std::path::Path;

pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub icc_profile: Option<Vec<u8>>,
}

pub fn decode(_src: &Path, _ext: ImageExt) -> Result<DecodedImage, EncodeError> {
    unimplemented!("decode stub — implemented in Task 1.3")
}
