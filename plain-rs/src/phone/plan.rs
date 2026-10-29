use super::number::NumberType;
use flate2::read::GzDecoder;
use regex::{Captures, Regex};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    io::Read,
    sync::{LazyLock, OnceLock},
};

#[derive(Deserialize)]
#[serde(from = "String")]
pub(super) struct Pattern {
    pub source: String,
    compiled: OnceLock<Regex>,
}

impl From<String> for Pattern {
    fn from(source: String) -> Self {
        Self {
            source,
            compiled: OnceLock::new(),
        }
    }
}

impl Pattern {
    fn regex(&self) -> &Regex {
        self.compiled
            .get_or_init(|| Regex::new(&self.source).expect("Invalid bundled numbering expression"))
    }
    pub fn matches(&self, number: &str) -> bool {
        !self.source.is_empty() && self.regex().is_match(number)
    }
    pub fn captures<'a>(&self, number: &'a str) -> Option<Captures<'a>> {
        if self.source.is_empty() {
            None
        } else {
            self.regex().captures(number)
        }
    }
    pub fn prefix<'a>(&self, number: &'a str) -> Option<&'a str> {
        self.captures(number)
            .map(|matched| &number[matched.get(0).unwrap().end()..])
    }
}

#[derive(Deserialize)]
struct Description {
    pattern: Pattern,
    lengths: Vec<usize>,
}
impl Description {
    fn matches(&self, number: &str) -> bool {
        self.lengths.contains(&number.len()) && self.pattern.matches(number)
    }
}

#[derive(Deserialize)]
pub(super) struct Plan {
    pub region: String,
    pub country_code: u16,
    pub international_prefix: Pattern,
    pub national_prefix: Pattern,
    pub transform: String,
    pub leading_digits: Pattern,
    pub general: Pattern,
    pub lengths: Vec<usize>,
    fixed_line: Description,
    mobile: Description,
    toll_free: Description,
    premium_rate: Description,
    shared_cost: Description,
    personal_number: Description,
    voip: Description,
    pager: Description,
    uan: Description,
    voicemail: Description,
}

impl Plan {
    pub fn kind(&self, number: &str) -> Option<NumberType> {
        if !self.general.matches(number) {
            return None;
        }
        for (kind, description) in [
            (NumberType::PremiumRate, &self.premium_rate),
            (NumberType::TollFree, &self.toll_free),
            (NumberType::SharedCost, &self.shared_cost),
            (NumberType::Voip, &self.voip),
            (NumberType::PersonalNumber, &self.personal_number),
            (NumberType::Pager, &self.pager),
            (NumberType::Uan, &self.uan),
            (NumberType::Voicemail, &self.voicemail),
        ] {
            if description.matches(number) {
                return Some(kind);
            }
        }
        match (self.fixed_line.matches(number), self.mobile.matches(number)) {
            (true, true) => Some(NumberType::FixedLineOrMobile),
            (true, false) => Some(NumberType::FixedLine),
            (false, true) => Some(NumberType::Mobile),
            _ => None,
        }
    }

    pub fn strip_national_prefix(&self, number: &mut String) {
        let Some(captures) = self.national_prefix.captures(number) else {
            return;
        };
        if captures.get(0).unwrap().is_empty() {
            return;
        }
        let first = captures.get(1);
        let second = captures.get(2);
        let mut stripped = String::new();
        if !self.transform.is_empty()
            && first.is_some()
            && (second.is_some_and(|second| !second.is_empty())
                || (second.is_none() && first.is_some_and(|first| !first.is_empty())))
        {
            captures.expand(&self.transform, &mut stripped);
        }
        stripped.push_str(&number[captures.get(0).unwrap().end()..]);
        if self.general.matches(number) && !self.general.matches(&stripped) {
            return;
        }
        if !self.lengths.contains(&stripped.len())
            && stripped.len() <= *self.lengths.last().unwrap()
        {
            return;
        }
        *number = stripped;
    }
}

#[derive(Deserialize)]
struct Data {
    plans: Vec<Plan>,
    decimal_zeroes: Vec<u32>,
}

pub(super) struct Plans {
    pub plans: Vec<Plan>,
    pub country_codes: BTreeMap<u16, Vec<usize>>,
    pub regions: BTreeMap<String, usize>,
    pub decimal_zeroes: Vec<u32>,
}

pub(super) static PLANS: LazyLock<Plans> = LazyLock::new(|| {
    let mut bytes = Vec::new();
    GzDecoder::new(include_bytes!("data/numbering.json.gz").as_slice())
        .read_to_end(&mut bytes)
        .expect("Invalid bundled numbering compression");
    let data: Data = serde_json::from_slice(&bytes).expect("Invalid bundled numbering data");
    let mut country_codes: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
    let mut regions = BTreeMap::new();
    for (index, plan) in data.plans.iter().enumerate() {
        country_codes
            .entry(plan.country_code)
            .or_default()
            .push(index);
        if plan.region != "001" {
            regions.insert(plan.region.clone(), index);
        }
    }
    Plans {
        plans: data.plans,
        country_codes,
        regions,
        decimal_zeroes: data.decimal_zeroes,
    }
});

#[cfg(test)]
#[path = "../../tests/unit/phone/plan.rs"]
mod tests;
