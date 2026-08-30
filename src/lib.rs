pub use wezat_core::*;
pub use wezat_macros::wz;

pub fn read<T: crate::Wezat<ReadArgs = ()>>(reader: &mut impl Reader) -> Result<T, Error> {
    T::from_bytes(reader)
}

#[cfg(test)]
mod tests {
    use crate as wezat;
    use std::io::{Seek, Write};
    use wezat_core::Wezat;

    #[test]
    fn ptr() -> Result<(), wezat::Error> {
        #[wezat::wz]
        pub struct BasicPointer {
            ptr: &bruh,
            bruh: u32,
        }

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
        #[wezat::wz]
        pub struct BasicPointer {
            ptr: &bruh,
            bruh: u32,
        }

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

    #[test]
    fn ptr_inverted() -> Result<(), wezat::Error> {
        #[wezat::wz]
        pub struct BasicPointerInverted {
            bruh: u32,
            ptr: &bruh,
        }

        let input = 67u32
            .to_le_bytes()
            .into_iter()
            .chain(0u32.to_le_bytes())
            .collect::<Vec<_>>();

        let mut reader = std::io::Cursor::new(&input);
        let bp = BasicPointerInverted::from_bytes(&mut reader)?;

        let mut output = vec![];
        bp.write_bytes(&mut std::io::Cursor::new(&mut output))?;

        assert_eq!(input, output);

        Ok(())
    }

    #[test]
    fn ptr_with_4_offset_inverted() -> Result<(), wezat::Error> {
        #[wezat::wz]
        pub struct BasicPointerInverted {
            bruh: u32,
            ptr: &bruh,
        }

        let input = 0u32
            .to_le_bytes()
            .into_iter()
            .chain(67u32.to_le_bytes())
            .chain(4u32.to_le_bytes())
            .collect::<Vec<_>>();

        let mut reader = std::io::Cursor::new(&input);
        reader.seek_relative(4)?;

        let bp = BasicPointerInverted::from_bytes(&mut reader)?;

        let mut output = vec![];
        let mut out_cur = &mut std::io::Cursor::new(&mut output);
        out_cur.write_all(&[0u8; 4])?;
        bp.write_bytes(&mut out_cur)?;

        assert_eq!(input, output);

        Ok(())
    }

    #[test]
    fn has_arrays() -> Result<(), wezat::Error> {
        #[wezat::wz]
        pub struct HasArrays {
            normal_data: [u8; 8],
            array_len: u32,
            normal_data_2: [u32; 2],
            array: [u16; array_len],
        }

        let input = (*b"abcdefgh")
            .into_iter()
            .chain(4u32.to_le_bytes())
            .chain([67u32, 69u32].into_iter().flat_map(|v| v.to_le_bytes()))
            .chain(
                [1u16, 2u16, 3u16, 4u16]
                    .into_iter()
                    .flat_map(|v| v.to_le_bytes()),
            )
            .collect::<Vec<_>>();

        let mut reader = std::io::Cursor::new(&input);

        let bp = HasArrays::from_bytes(&mut reader)?;

        let mut output = vec![];
        let mut out_cur = &mut std::io::Cursor::new(&mut output);
        bp.write_bytes(&mut out_cur)?;

        assert_eq!(input, output);

        Ok(())
    }

    #[test]
    fn with_str() -> Result<(), wezat::Error> {
        #[wezat::wz(len = LEN)]
        pub struct WithStr {
            normal_data: SECTION_1::i,
            some_data_1: u32,
            some_data_2: u32,
            bruh: [u8; 4],
            SECTION_1: [wezat::CString; LEN],
        }

        let mut input = [
            // 1
            0x20,
            0,
            0,
            0xffffffffu32,
            // 2
            0x28,
            0,
            0,
            0xffffffffu32,
        ]
        .into_iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();

        input.extend(b"string_1");
        input.extend(b"string_2");

        assert_eq!(input.len(), 0x30);

        let mut reader = std::io::Cursor::new(&input);

        let v = WithStr::from_bytes(&mut reader)?;

        let mut output = vec![];
        let mut out_cur = &mut std::io::Cursor::new(&mut output);
        v.write_bytes(&mut out_cur)?;

        assert_eq!(input, output);

        Ok(())
    }
}
