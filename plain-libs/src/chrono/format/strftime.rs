use super::{BAD_FORMAT, ParseError};
use super::{Fixed, InternalInternal, Item, Numeric, Pad};
use super::{fixed, internal_fixed, num, num0, nums};
#[derive(Clone, Debug)]
#[cfg_attr(any(), derive(defmt::Format))]
pub struct StrftimeItems<'a> {
    remainder: &'a str,
    queue: &'static [Item<'static>],
    lenient: bool,
}
impl<'a> StrftimeItems<'a> {
    #[must_use]
    pub const fn new(s: &'a str) -> StrftimeItems<'a> {
        StrftimeItems {
            remainder: s,
            queue: &[],
            lenient: false,
        }
    }

    pub fn parse(self) -> Result<Vec<Item<'a>>, ParseError> {
        self.into_iter()
            .map(|item| match item == Item::Error {
                false => Ok(item),
                true => Err(BAD_FORMAT),
            })
            .collect()
    }

    fn parse_next_item(&mut self, mut remainder: &'a str) -> Option<(&'a str, Item<'a>)> {
        use InternalInternal::*;
        use Item::{Literal, Space};
        use Numeric::*;
        let (original, mut remainder) = match remainder.chars().next()? {
            '%' => (remainder, &remainder[1..]),
            c if c.is_whitespace() => {
                let nextspec = remainder
                    .find(|c: char| !c.is_whitespace())
                    .unwrap_or(remainder.len());
                assert!(nextspec > 0);
                let item = Space(&remainder[..nextspec]);
                remainder = &remainder[nextspec..];
                return Some((remainder, item));
            }
            _ => {
                let nextspec = remainder
                    .find(|c: char| c.is_whitespace() || c == '%')
                    .unwrap_or(remainder.len());
                assert!(nextspec > 0);
                let item = Literal(&remainder[..nextspec]);
                remainder = &remainder[nextspec..];
                return Some((remainder, item));
            }
        };
        macro_rules! next {
            () => {
                match remainder.chars().next() {
                    Some(x) => {
                        remainder = &remainder[x.len_utf8()..];
                        x
                    }
                    None => return Some((remainder, self.error(original, remainder))),
                }
            };
        }
        let spec = next!();
        let pad_override = match spec {
            '-' => Some(Pad::None),
            '0' => Some(Pad::Zero),
            '_' => Some(Pad::Space),
            _ => None,
        };
        let is_alternate = spec == '#';
        let spec = if pad_override.is_some() || is_alternate {
            next!()
        } else {
            spec
        };
        if is_alternate && !HAVE_ALTERNATES.contains(spec) {
            return Some((remainder, self.error(original, remainder)));
        }
        macro_rules ! queue { [$ head : expr , $ ($ tail : expr) ,+ $ (,) *] => ({ const QUEUE : &'static [Item <'static >] = & [$ ($ tail) ,+] ; self . queue = QUEUE ; $ head }) }
        macro_rules! queue_from_slice {
            ($ slice : expr) => {{
                self.queue = &$slice[1..];
                $slice[0].clone()
            }};
        }
        let item = match spec {
            'A' => fixed(Fixed::LongWeekdayName),
            'B' => fixed(Fixed::LongMonthName),
            'C' => num0(YearDiv100),
            'D' => {
                queue![
                    num0(Month),
                    Literal("/"),
                    num0(Day),
                    Literal("/"),
                    num0(YearMod100)
                ]
            }
            'F' => queue![
                num0(Year),
                Literal("-"),
                num0(Month),
                Literal("-"),
                num0(Day)
            ],
            'G' => num0(IsoYear),
            'H' => num0(Hour),
            'I' => num0(Hour12),
            'M' => num0(Minute),
            'P' => fixed(Fixed::LowerAmPm),
            'R' => queue![num0(Hour), Literal(":"), num0(Minute)],
            'S' => num0(Second),
            'T' => {
                queue![
                    num0(Hour),
                    Literal(":"),
                    num0(Minute),
                    Literal(":"),
                    num0(Second)
                ]
            }
            'U' => num0(WeekFromSun),
            'V' => num0(IsoWeek),
            'W' => num0(WeekFromMon),
            'X' => queue_from_slice!(T_FMT),
            'Y' => num0(Year),
            'Z' => fixed(Fixed::TimezoneName),
            'a' => fixed(Fixed::ShortWeekdayName),
            'b' | 'h' => fixed(Fixed::ShortMonthName),
            'c' => queue_from_slice!(D_T_FMT),
            'd' => num0(Day),
            'e' => nums(Day),
            'f' => num0(Nanosecond),
            'g' => num0(IsoYearMod100),
            'j' => num0(Ordinal),
            'k' => nums(Hour),
            'l' => nums(Hour12),
            'm' => num0(Month),
            'n' => Space("\n"),
            'p' => fixed(Fixed::UpperAmPm),
            'q' => num(Quarter),
            'r' => queue_from_slice!(T_FMT_AMPM),
            's' => num(Timestamp),
            't' => Space("\t"),
            'u' => num(WeekdayFromMon),
            'v' => {
                queue![
                    nums(Day),
                    Literal("-"),
                    fixed(Fixed::ShortMonthName),
                    Literal("-"),
                    num0(Year)
                ]
            }
            'w' => num(NumDaysFromSun),
            'x' => queue_from_slice!(D_FMT),
            'y' => num0(YearMod100),
            'z' => {
                if is_alternate {
                    internal_fixed(TimezoneOffsetPermissive)
                } else {
                    fixed(Fixed::TimezoneOffset)
                }
            }
            '+' => fixed(Fixed::RFC3339),
            ':' => {
                if remainder.starts_with("::z") {
                    remainder = &remainder[3..];
                    fixed(Fixed::TimezoneOffsetTripleColon)
                } else if remainder.starts_with(":z") {
                    remainder = &remainder[2..];
                    fixed(Fixed::TimezoneOffsetDoubleColon)
                } else if remainder.starts_with('z') {
                    remainder = &remainder[1..];
                    fixed(Fixed::TimezoneOffsetColon)
                } else {
                    self.error(original, remainder)
                }
            }
            '.' => match next!() {
                '3' => match next!() {
                    'f' => fixed(Fixed::Nanosecond3),
                    _ => self.error(original, remainder),
                },
                '6' => match next!() {
                    'f' => fixed(Fixed::Nanosecond6),
                    _ => self.error(original, remainder),
                },
                '9' => match next!() {
                    'f' => fixed(Fixed::Nanosecond9),
                    _ => self.error(original, remainder),
                },
                'f' => fixed(Fixed::Nanosecond),
                _ => self.error(original, remainder),
            },
            '3' => match next!() {
                'f' => internal_fixed(Nanosecond3NoDot),
                _ => self.error(original, remainder),
            },
            '6' => match next!() {
                'f' => internal_fixed(Nanosecond6NoDot),
                _ => self.error(original, remainder),
            },
            '9' => match next!() {
                'f' => internal_fixed(Nanosecond9NoDot),
                _ => self.error(original, remainder),
            },
            '%' => Literal("%"),
            _ => self.error(original, remainder),
        };
        if let Some(new_pad) = pad_override {
            match item {
                Item::Numeric(ref kind, _pad) if self.queue.is_empty() => {
                    Some((remainder, Item::Numeric(kind.clone(), new_pad)))
                }
                _ => Some((remainder, self.error(original, remainder))),
            }
        } else {
            Some((remainder, item))
        }
    }
    fn error<'b>(&mut self, original: &'b str, remainder: &'b str) -> Item<'b> {
        match self.lenient {
            false => Item::Error,
            true => Item::Literal(&original[..original.len() - remainder.len()]),
        }
    }
}
impl<'a> Iterator for StrftimeItems<'a> {
    type Item = Item<'a>;
    fn next(&mut self) -> Option<Item<'a>> {
        if let Some((item, remainder)) = self.queue.split_first() {
            self.queue = remainder;
            return Some(item.clone());
        }
        let (remainder, item) = self.parse_next_item(self.remainder)?;
        self.remainder = remainder;
        Some(item)
    }
}
static D_FMT: &[Item<'static>] = &[
    num0(Numeric::Month),
    Item::Literal("/"),
    num0(Numeric::Day),
    Item::Literal("/"),
    num0(Numeric::YearMod100),
];
static D_T_FMT: &[Item<'static>] = &[
    fixed(Fixed::ShortWeekdayName),
    Item::Space(" "),
    fixed(Fixed::ShortMonthName),
    Item::Space(" "),
    nums(Numeric::Day),
    Item::Space(" "),
    num0(Numeric::Hour),
    Item::Literal(":"),
    num0(Numeric::Minute),
    Item::Literal(":"),
    num0(Numeric::Second),
    Item::Space(" "),
    num0(Numeric::Year),
];
static T_FMT: &[Item<'static>] = &[
    num0(Numeric::Hour),
    Item::Literal(":"),
    num0(Numeric::Minute),
    Item::Literal(":"),
    num0(Numeric::Second),
];
static T_FMT_AMPM: &[Item<'static>] = &[
    num0(Numeric::Hour12),
    Item::Literal(":"),
    num0(Numeric::Minute),
    Item::Literal(":"),
    num0(Numeric::Second),
    Item::Space(" "),
    fixed(Fixed::UpperAmPm),
];
const HAVE_ALTERNATES: &str = "z";
