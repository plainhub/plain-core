#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvisoryRating {
    Inoffensive,

    Explicit,

    Clean,
}

impl AdvisoryRating {
    pub fn as_u8(&self) -> u8 {
        match self {
            AdvisoryRating::Inoffensive => 0,
            AdvisoryRating::Explicit => 1,
            AdvisoryRating::Clean => 2,
        }
    }
}

impl TryFrom<u8> for AdvisoryRating {
    type Error = u8;

    fn try_from(input: u8) -> Result<Self, Self::Error> {
        match input {
            0 => Ok(Self::Inoffensive),
            1 | 4 => Ok(Self::Explicit),
            2 => Ok(Self::Clean),
            value => Err(value),
        }
    }
}
