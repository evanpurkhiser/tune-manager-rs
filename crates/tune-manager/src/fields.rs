use regex::Regex;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{convert::Infallible, fmt, str, sync::LazyLock};

static COUNT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?P<number>\d+) */ **(?P<total>\d+)").unwrap());

#[derive(Debug, Clone, PartialEq)]
pub struct Count {
    /// The left hand side of the count
    pub number: u8,

    /// The right hand "total" side of the count
    pub total: u8,
}

#[derive(Debug, Clone, PartialEq, SerializeDisplay, DeserializeFromStr)]
pub enum CountField {
    Valid(Count),
    Invalid(String),
}

impl str::FromStr for CountField {
    type Err = Infallible;

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
    use std::convert::Infallible;

    use super::{Count, CountField};

    #[test]
    fn test_count_field_parse() -> Result<(), Infallible> {
        let value = "1 / 10".parse();
        let expect = CountField::Valid(Count {
            number: 1,
            total: 10,
        });
        assert_eq!(value, Ok(expect));
        assert_eq!(value?.to_string(), "1/10");
        Ok(())
    }

    #[test]
    fn test_count_field_serialize() -> Result<(), serde_json::Error> {
        let value: CountField = serde_json::from_str("\"1 / 10\"")?;
        let expect = CountField::Valid(Count {
            number: 1,
            total: 10,
        });
        assert_eq!(value, expect);
        assert_eq!(serde_json::to_string(&value)?, "\"1/10\"");
        Ok(())
    }
}
