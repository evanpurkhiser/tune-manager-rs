use lazy_static::lazy_static;
use regex::Regex;
use std::{fmt, str};

lazy_static! {
    static ref COUNT_REGEX: Regex = Regex::new(r"(?P<number>\d+) */ **(?P<total>\d+)").unwrap();
}

#[derive(Debug)]
pub struct Count {
    /// The left hand side of the count
    pub number: u8,

    /// The right hand "total" side of the count
    pub total: u8,
}

#[derive(Debug)]
pub enum CountField {
    Valid(Count),
    Invalid(String),
}

impl str::FromStr for CountField {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(c) = COUNT_REGEX.captures(s) {
            let number = c.name("number").unwrap().as_str().parse().unwrap();
            let total = c.name("total").unwrap().as_str().parse().unwrap();
            Ok(Self::Valid(Count { number, total }))
        } else {
            Ok(Self::Invalid(s.to_owned()))
        }
    }
}

impl fmt::Display for CountField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid(c) => write!(f, "{}/{}", c.number, c.total),
            Self::Invalid(s) => f.write_str(s),
        }
    }
}
