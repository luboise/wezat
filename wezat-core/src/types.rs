pub type NullTerminatedString = TerminatedString<{ 0 as char }>;

/// A string terminated by a specific character
///
/// Does not contain the terminator after parsing
pub struct TerminatedString<const C: char>(pub String);

impl<const C: char> TryFrom<&str> for TerminatedString<C> {
    type Error = crate::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if !s.contains(C) {
            return Err("terminating char not found".into());
        }

        Ok(Self(s.chars().take_while(|c| *c != C).collect()))
    }
}

impl<const C: char> TryFrom<&[u8]> for TerminatedString<C> {
    type Error = crate::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if !bytes.contains(&(C as u8)) {
            return Err("terminating char not found".into());
        }

        let s = String::from_utf8(
            bytes
                .iter()
                .take_while(|b| **b != C as u8)
                .copied()
                .collect(),
        )?;

        Ok(Self(s))
    }
}

impl<const C: char, const L: usize> TryFrom<&[u8; L]> for TerminatedString<C> {
    type Error = crate::Error;

    fn try_from(bytes: &[u8; L]) -> Result<Self, Self::Error> {
        if !bytes.contains(&(C as u8)) {
            return Err("terminating char not found".into());
        }

        let s = String::from_utf8(
            bytes
                .iter()
                .take_while(|b| **b != C as u8)
                .copied()
                .collect(),
        )?;

        Ok(Self(s))
    }
}

pub struct OffsetTable {}
