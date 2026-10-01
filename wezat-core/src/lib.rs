// TODO: Replace this with an actual error type
pub type Error = Box<dyn std::error::Error>;

mod types;
pub use types::*;

pub trait Reader: std::io::Seek + std::io::Read {}
impl<T> Reader for T where T: std::io::Seek + std::io::Read {}

pub trait Writer: std::io::Seek + std::io::Write {}
impl<T> Writer for T where T: std::io::Seek + std::io::Write {}

/// A wezat value, which can be serialised forwards and backwards, as well as written into a
/// reader/writer.
pub trait Wezat: Sized {
    const MIN_SIZE: usize;

    type ReadArgs;
    type WriteArgs;

    /// calculates the size of the value serialised
    fn size(&self) -> usize {
        Self::MIN_SIZE
    }

    fn from_bytes_ctx(
        ctx: &ReadContext<Self::ReadArgs>,
        reader: &mut impl Reader,
    ) -> Result<Self, Error>;

    fn write_bytes_ctx(
        &self,
        ctx: &WriteContext<Self::WriteArgs>,
        writer: &mut impl Writer,
    ) -> Result<(), Error>;

    // convenience methods for trait implementations
    fn from_bytes(reader: &mut impl Reader) -> Result<Self, Error>
    where
        Self: Wezat<ReadArgs = ()>,
    {
        Self::from_bytes_ctx(&ReadContext::default(), reader)
    }

    fn write_bytes(&self, writer: &mut impl Writer) -> Result<(), Error>
    where
        Self: Wezat<WriteArgs = ()>,
    {
        self.write_bytes_ctx(&WriteContext::default(), writer)
    }
}

#[derive(Default, Clone)]
pub struct GlobalReadContextInner;

impl<T: Default> Default for ReadContext<T> {
    fn default() -> Self {
        Self {
            args: Default::default(),
            global: Default::default(),
        }
    }
}

impl<T: Clone> Clone for ReadContext<T> {
    fn clone(&self) -> Self {
        Self {
            args: self.args.clone(),
            global: self.global.clone(),
        }
    }
}

pub struct ReadContext<T> {
    /// direct context used by the current operation
    pub args: T,
    pub global: std::cell::RefCell<GlobalReadContextInner>,
}

#[derive(Default)]
pub struct GlobalWriteContext;

pub struct WriteContext<T> {
    /// direct context used by the current operation
    pub args: T,
    pub global: GlobalWriteContext,
}

impl<T: Default> Default for WriteContext<T> {
    fn default() -> Self {
        Self {
            args: Default::default(),
            global: Default::default(),
        }
    }
}

macro_rules! impl_wezat_primitive {
    ($t:ty) => {
        impl Wezat for $t {
            const MIN_SIZE: usize = size_of::<$t>();

            type ReadArgs = ();
            type WriteArgs = ();

            fn from_bytes_ctx(
                _: &ReadContext<()>,
                reader: &mut impl Reader,
            ) -> Result<Self, Error> {
                let mut bytes = [0u8; Self::MIN_SIZE];
                reader.read_exact(&mut bytes)?;
                Ok(Self::from_le_bytes(bytes))
            }

            fn write_bytes_ctx(
                &self,
                _: &WriteContext<()>,
                writer: &mut impl Writer,
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
    const MIN_SIZE: usize = T::MIN_SIZE * C;

    type ReadArgs = T::ReadArgs;
    type WriteArgs = T::WriteArgs;

    fn from_bytes_ctx(
        ctx: &ReadContext<Self::ReadArgs>,
        reader: &mut impl Reader,
    ) -> Result<Self, Error> {
        let mut ret = [T::default(); C];

        for elem in ret.iter_mut().take(C) {
            *elem = T::from_bytes_ctx(ctx, reader)?;
        }

        Ok(ret)
    }

    fn write_bytes_ctx(
        &self,
        ctx: &WriteContext<Self::WriteArgs>,
        writer: &mut impl Writer,
    ) -> Result<(), Error> {
        for item in self {
            item.write_bytes_ctx(ctx, writer)?;
        }
        Ok(())
    }
}

impl<const C: char> Wezat for TerminatedString<C> {
    const MIN_SIZE: usize = 0;

    type ReadArgs = ();
    type WriteArgs = ();

    fn from_bytes_ctx(
        _: &ReadContext<Self::ReadArgs>,
        reader: &mut impl Reader,
    ) -> Result<Self, Error> {
        // TODO: make it read directly from the reader?
        let c_as_u8 = C as u8;

        let mut bytes = vec![];

        loop {
            let new_byte = u8::from_bytes(reader)?;
            bytes.push(new_byte);

            if new_byte == c_as_u8 {
                break;
            }
        }

        Self::try_from(bytes.as_slice())
    }

    fn write_bytes_ctx(
        &self,
        _: &WriteContext<Self::WriteArgs>,
        writer: &mut impl Writer,
    ) -> Result<(), Error> {
        writer.write_all(self.0.as_bytes())?;
        (C as u8).write_bytes(writer)?;

        Ok(())
    }
}

impl Wezat for () {
    const MIN_SIZE: usize = 0;

    type ReadArgs = ();
    type WriteArgs = ();

    fn from_bytes_ctx(_: &ReadContext<Self::ReadArgs>, _: &mut impl Reader) -> Result<Self, Error> {
        Ok(())
    }

    fn write_bytes_ctx(
        &self,
        _: &WriteContext<Self::WriteArgs>,
        _: &mut impl Writer,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl Wezat for Option<()> {
    const MIN_SIZE: usize = 0;

    type ReadArgs = ();
    type WriteArgs = ();

    fn from_bytes_ctx(_: &ReadContext<Self::ReadArgs>, _: &mut impl Reader) -> Result<Self, Error> {
        Ok(Some(()))
    }

    fn write_bytes_ctx(
        &self,
        _: &WriteContext<Self::WriteArgs>,
        _: &mut impl Writer,
    ) -> Result<(), Error> {
        Ok(())
    }
}
