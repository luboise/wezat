// TODO: Replace this with an actual error type
pub type Error = Box<dyn std::error::Error>;

pub trait Reader: std::io::Seek + std::io::BufRead {}
impl<T> Reader for T where T: std::io::Seek + std::io::BufRead {}

pub trait Writer: std::io::Seek + std::io::Write {}
impl<T> Writer for T where T: std::io::Seek + std::io::Write {}

/// A wezat value, which can be serialised forwards and backwards, as well as written into a
/// reader/writer.
pub trait Wezat: Sized {
    /// calculates the size of the value serialised
    type ReadArgs;
    fn from_bytes_with_args(reader: &mut impl Reader, args: &Self::ReadArgs)
    -> Result<Self, Error>;

    type WriteArgs;
    fn write_bytes_with_args(
        &self,
        writer: &mut impl Writer,
        args: &Self::WriteArgs,
    ) -> Result<(), Error>;

    // extra methods for implementations which take no args
    #[inline]
    fn from_bytes(reader: &mut impl Reader) -> Result<Self, Error>
    where
        Self: Wezat<ReadArgs = ()>,
    {
        Self::from_bytes_with_args(reader, &())
    }

    #[inline]
    fn write_bytes(&self, writer: &mut impl Writer) -> Result<(), Error>
    where
        Self: Wezat<WriteArgs = ()>,
    {
        self.write_bytes_with_args(writer, &())
    }
}

macro_rules! impl_wezat_primitive {
    ($t:ty) => {
        impl Wezat for $t {
            type ReadArgs = ();
            type WriteArgs = ();
            fn from_bytes_with_args(
                reader: &mut impl Reader,
                _: &Self::ReadArgs,
            ) -> Result<Self, Error> {
                let mut bytes = [0u8; size_of::<$t>()];
                reader.read_exact(&mut bytes)?;
                Ok(Self::from_le_bytes(bytes))
            }

            fn write_bytes_with_args(
                &self,
                writer: &mut impl Writer,
                _: &Self::WriteArgs,
            ) -> Result<(), Error> {
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
    type ReadArgs = T::ReadArgs;
    fn from_bytes_with_args(
        reader: &mut impl Reader,
        args: &Self::ReadArgs,
    ) -> Result<Self, Error> {
        let mut ret = [T::default(); C];

        for elem in ret.iter_mut().take(C) {
            *elem = T::from_bytes_with_args(reader, args)?;
        }

        Ok(ret)
    }

    type WriteArgs = T::WriteArgs;
    fn write_bytes_with_args(
        &self,
        writer: &mut impl Writer,
        args: &Self::WriteArgs,
    ) -> Result<(), Error> {
        for item in self {
            item.write_bytes_with_args(writer, args)?;
        }
        Ok(())
    }
}

pub struct CString {
    pub s: String,
}

impl<T: AsRef<str>> From<T> for CString {
    fn from(value: T) -> Self {
        Self {
            s: value.as_ref().to_owned(),
        }
    }
}

impl Wezat for CString {
    type ReadArgs = ();
    fn from_bytes_with_args(reader: &mut impl Reader, _: &Self::ReadArgs) -> Result<Self, Error> {
        let mut buf = vec![];

        let _ = reader.read_until(0, &mut buf)?;

        if let Some(0) = buf.last() {
            buf.pop();
        }

        Ok(Self {
            s: String::from_utf8(buf)?,
        })
    }

    type WriteArgs = ();
    fn write_bytes_with_args(
        &self,
        writer: &mut impl Writer,
        _: &Self::WriteArgs,
    ) -> Result<(), Error> {
        writer.write_all(self.s.as_bytes())?;
        writer.write_all(&[0u8])?;
        Ok(())
    }
}
