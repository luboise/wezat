pub use wezat_core::*;
pub use wezat_macros::wz;

pub fn read<T: crate::Wezat>(reader: &mut impl Reader) -> Result<T, Error> {
    T::from_bytes(reader)
}

#[cfg(test)]
mod tests {
    use crate as wezat;
    use std::io::{Seek, Write};
    use wezat_core::Wezat;

    #[wezat::wz]
    pub struct BasicPointer {
        ptr: &bruh,
        bruh: u32,
    }

    #[test]
    fn ptr() -> Result<(), wezat::Error> {
        let input = 4u32
            .to_le_bytes()
            .into_iter()
            .chain(67u32.to_le_bytes())
            .collect::<Vec<_>>();

        let mut reader = std::io::Cursor::new(&input);
        let bp = BasicPointer::from_bytes(&mut reader)?;

        let mut output = vec![];
        bp.write_bytes(&mut std::io::Cursor::new(&mut output))?;

        assert_eq!(input, output);

        Ok(())
    }

    #[test]
    fn ptr_with_4_offset() -> Result<(), wezat::Error> {
        let input = 0u32
            .to_le_bytes()
            .into_iter()
            .chain(8u32.to_le_bytes())
            .chain(67u32.to_le_bytes())
            .collect::<Vec<_>>();

        let mut reader = std::io::Cursor::new(&input);
        reader.seek_relative(4)?;

        let bp = BasicPointer::from_bytes(&mut reader)?;

        let mut output = vec![];
        let mut out_cur = &mut std::io::Cursor::new(&mut output);
        out_cur.write_all(&[0u8; 4])?;
        bp.write_bytes(&mut out_cur)?;

        assert_eq!(input, output);

        Ok(())
    }
}
