// TODO: Replace this with an actual error type
pub type Error = Box<dyn std::error::Error>;

pub trait Reader: std::io::Seek + std::io::Read {}
impl<T> Reader for T where T: std::io::Seek + std::io::Read {}

pub trait Writer: std::io::Seek + std::io::Write {}
impl<T> Writer for T where T: std::io::Seek + std::io::Write {}

/// A wezat value, which can be serialised forwards and backwards, as well as written into a
/// reader/writer.
pub trait Wezat: Sized {
    const MIN_SIZE: usize;

    /// calculates the size of the value serialised
    fn size(&self) -> usize {
        Self::MIN_SIZE
    }
    fn from_bytes(reader: &mut impl Reader) -> Result<Self, Error>;
    fn write_bytes(&self, writer: &mut impl Writer) -> Result<(), Error>;
}

macro_rules! impl_wezat_primitive {
    ($t:ty) => {
        impl Wezat for $t {
            const MIN_SIZE: usize = size_of::<$t>();

            fn from_bytes(reader: &mut impl Reader) -> Result<Self, Error> {
                let mut bytes = [0u8; Self::MIN_SIZE];
                reader.read_exact(&mut bytes)?;
                Ok(Self::from_le_bytes(bytes))
            }

            fn write_bytes(&self, writer: &mut impl Writer) -> Result<(), Error> {
                writer.write_all(&self.to_le_bytes())?;
                Ok(())
            }
        }
    };
}

impl_wezat_primitive!(u8);
impl_wezat_primitive!(u16);
impl_wezat_primitive!(u32);
impl_wezat_primitive!(u64);
impl_wezat_primitive!(i8);
impl_wezat_primitive!(i16);
impl_wezat_primitive!(i32);
impl_wezat_primitive!(i64);
impl_wezat_primitive!(f32);
impl_wezat_primitive!(f64);

impl<T: Wezat + Default + Copy, const C: usize> Wezat for [T; C] {
    const MIN_SIZE: usize = T::MIN_SIZE * C;

    fn from_bytes(reader: &mut impl Reader) -> Result<Self, Error> {
        let mut ret = [T::default(); C];

        for elem in ret.iter_mut().take(C) {
            *elem = T::from_bytes(reader)?;
        }

        Ok(ret)
    }

    fn write_bytes(&self, writer: &mut impl Writer) -> Result<(), Error> {
        todo!()
    }
}
