pub use wezat_core::*;
pub use wezat_macros::wz;

pub fn read<T: crate::Wezat>(reader: &mut impl Reader) -> Result<T, Error> {
    T::from_bytes(reader)
}
