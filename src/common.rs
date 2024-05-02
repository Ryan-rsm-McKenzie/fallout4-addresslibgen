use nonmax::{
    NonMaxU64,
    TryFromIntError,
};
use std::{
    fmt::{
        self,
        Display,
        Formatter,
    },
    ops::Index,
};

#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Id(NonMaxU64);

impl Id {
    pub fn next(self) -> Id {
        Self(NonMaxU64::new(self.0.get() + 1).expect("id is too large to fit within range"))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.get())
    }
}

impl TryFrom<u64> for Id {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Offset(pub u32);

impl Display for Offset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:X}", self.0)
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version([u16; 4]);

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}.{}", self[0], self[1], self[2], self[3])
    }
}

impl Index<usize> for Version {
    type Output = u16;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl TryFrom<(&str, &str, &str)> for Version {
    type Error = anyhow::Error;

    fn try_from(value: (&str, &str, &str)) -> anyhow::Result<Self> {
        Ok(Self([
            value.0.parse()?,
            value.1.parse()?,
            value.2.parse()?,
            0,
        ]))
    }
}

impl TryFrom<(&str, &str, &str, &str)> for Version {
    type Error = anyhow::Error;

    fn try_from(value: (&str, &str, &str, &str)) -> anyhow::Result<Self> {
        Ok(Self([
            value.0.parse()?,
            value.1.parse()?,
            value.2.parse()?,
            value.3.parse()?,
        ]))
    }
}
