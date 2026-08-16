pub use wezat_core::*;
pub use wezat_macros::wz;

pub fn read<T: crate::Wezat>(reader: &mut impl Reader) -> Result<T, Error> {
    T::from_bytes(reader)
}

#[cfg(test)]
mod tests {
    use wezat_core::Wezat;

    use crate as wezat;

    pub struct BasicPointer {
        bruh: u32,
    }

    impl wezat::Wezat for BasicPointer {
        const MIN_SIZE: usize = 0;

        fn from_bytes(reader: &mut impl wezat_core::Reader) -> Result<Self, wezat_core::Error> {
            let bruh_ptr: u32 = Wezat::from_bytes(reader)?;
            let bruh = {
                let restore_pos = reader.stream_position()?;
                reader.seek(std::io::SeekFrom::Start(bruh_ptr.into()))?;
                let value = Wezat::from_bytes(reader)?;
                reader.seek(std::io::SeekFrom::Start(restore_pos))?;
                value
            };

            Ok(Self { bruh })
        }

        fn write_bytes(
            &self,
            writer: &mut impl wezat_core::Writer,
        ) -> Result<(), wezat_core::Error> {
            4u32.write_bytes(writer)?;
            self.bruh.write_bytes(writer)?;

            Ok(())
        }
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
}
