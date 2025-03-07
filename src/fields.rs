use regex::Regex;
use std::{fmt, str, sync::LazyLock};

static COUNT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?P<number>\d+) */ **(?P<total>\d+)").unwrap());

#[derive(Debug, PartialEq)]
pub struct Count {
    /// The left hand side of the count
    pub number: u8,

    /// The right hand "total" side of the count
    pub total: u8,
}

#[derive(Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::{Count, CountField};

    #[test]
    fn test_count_field() {
        let value = "1 / 10".parse();
        let expect = CountField::Valid(Count {
            number: 1,
            total: 10,
        });
        assert_eq!(value, Ok(expect));
        assert_eq!(value.unwrap().to_string(), "1/10");
    }
}
