use super::{decode::DecodedImage, EncodeError, EncodedFile};

pub fn encode(_decoded: &DecodedImage, _quality: u32) -> Result<EncodedFile, EncodeError> {
    unimplemented!("webp encode stub — implemented in Task 1.8")
}
