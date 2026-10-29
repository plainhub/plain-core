use super::{
    plan::{PLANS, Plan},
    syntax,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NumberType {
    FixedLine,
    Mobile,
    FixedLineOrMobile,
    TollFree,
    PremiumRate,
    SharedCost,
    PersonalNumber,
    Voip,
    Pager,
    Uan,
    Voicemail,
}

pub(super) struct Number {
    pub country_code: u16,
    pub national: String,
    pub kind: NumberType,
}

pub(super) fn regions(code: u16) -> impl Iterator<Item = &'static Plan> {
    PLANS
        .country_codes
        .get(&code)
        .into_iter()
        .flatten()
        .map(|index| &PLANS.plans[*index])
}

pub(super) fn main_region(number: &Number) -> &'static str {
    &PLANS.plans[PLANS.country_codes[&number.country_code][0]].region
}

pub(super) fn parse(input: &str, region: &str) -> Option<Number> {
    let normalized = syntax::prepare(input)?;
    let default = PLANS
        .regions
        .get(&region.to_ascii_uppercase())
        .map(|index| &PLANS.plans[*index]);
    let mut national = normalized.digits;
    let idd = default
        .and_then(|plan| plan.international_prefix.prefix(&national))
        .filter(|number| !number.starts_with('0'))
        .map(str::to_owned);
    let country_code = if normalized.plus || idd.is_some() {
        if !normalized.plus {
            national = idd?;
        }
        let (code, number) = extract_code(&national).or_else(|| {
            if !normalized.plus {
                return None;
            }
            let number = default?.international_prefix.prefix(&national)?;
            extract_code(number)
        })?;
        national = number.to_owned();
        code
    } else {
        let plan = default?;
        let code = plan.country_code.to_string();
        if let Some(number) = national.strip_prefix(&code) {
            let mut candidate = number.to_owned();
            plan.strip_national_prefix(&mut candidate);
            if (!plan.general.matches(&national) && plan.general.matches(&candidate))
                || national.len() > *plan.lengths.last().unwrap()
            {
                national = candidate;
            }
        }
        plan.country_code
    };
    let prefix_plan = default
        .filter(|plan| plan.country_code == country_code)
        .or_else(|| regions(country_code).next())?;
    prefix_plan.strip_national_prefix(&mut national);
    if !(2..=17).contains(&national.len()) {
        return None;
    }
    let group = &PLANS.country_codes[&country_code];
    for index in group {
        let plan = &PLANS.plans[*index];
        if group.len() == 1
            || (!plan.leading_digits.source.is_empty() && plan.leading_digits.matches(&national))
        {
            return Some(Number {
                country_code,
                kind: plan.kind(&national)?,
                national,
            });
        }
        if plan.leading_digits.source.is_empty() {
            if let Some(kind) = plan.kind(&national) {
                return Some(Number {
                    country_code,
                    national,
                    kind,
                });
            }
        }
    }
    None
}

fn extract_code(number: &str) -> Option<(u16, &str)> {
    if number.starts_with('0') {
        return None;
    }
    let mut code = 0;
    for (index, digit) in number.bytes().take(3).enumerate() {
        code = code * 10 + u16::from(digit - b'0');
        if PLANS.country_codes.contains_key(&code) {
            return Some((code, &number[index + 1..]));
        }
    }
    None
}

#[cfg(test)]
#[path = "../../tests/unit/phone/number.rs"]
mod tests;
