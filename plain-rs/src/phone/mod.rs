mod data;
mod locale;
mod number;
mod plan;
mod syntax;

use locale::Locale;
use number::{Number, NumberType};
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub country: String,
    pub number_type: String,
    pub carrier: String,
    pub description: String,
}

pub fn default_region<'a>(network: &'a str, sim: &'a str, locale: &'a str) -> &'a str {
    [network, sim, locale]
        .into_iter()
        .find(|region| !region.trim().is_empty())
        .unwrap_or_default()
}

#[derive(Default)]
pub struct Service {
    data: data::Data,
    cache: Mutex<VecDeque<(String, Option<Metadata>)>>,
    #[cfg(test)]
    resolutions: std::sync::atomic::AtomicUsize,
}

impl Service {
    pub fn lookup(&self, number: &str, region: &str, language_tag: &str) -> Option<Metadata> {
        if number.trim().is_empty() {
            return None;
        }
        let key = format!("{region}\0{language_tag}\0{number}");
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(index) = cache.iter().position(|(cached, _)| *cached == key) {
                let entry = cache.remove(index).unwrap();
                let result = entry.1.clone();
                cache.push_back(entry);
                return result;
            }
        }
        let result = self.resolve(number, region, language_tag);
        if key.len() <= 4096 {
            let mut cache = self.cache.lock().unwrap();
            cache.retain(|(cached, _)| *cached != key);
            cache.push_back((key, result.clone()));
            while cache.len() > 1024 {
                cache.pop_front();
            }
        }
        result
    }

    fn resolve(&self, number: &str, region: &str, language_tag: &str) -> Option<Metadata> {
        #[cfg(test)]
        self.resolutions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let parsed = number::parse(number, region)?;
        let country = number::main_region(&parsed).to_owned();
        let number_type = parsed.kind;
        let locale = Locale::parse(language_tag);
        let carrier = if matches!(
            number_type,
            NumberType::Mobile | NumberType::FixedLineOrMobile | NumberType::Pager
        ) {
            self.prefix("carrier", &parsed, &locale)
        } else {
            String::new()
        };
        let geographical = self.geographical_number(&parsed);
        let description = self.prefix("geo", geographical.as_ref().unwrap_or(&parsed), &locale);
        let description = if description.is_empty() {
            self.country_name(&parsed, &locale)
        } else {
            description
        };
        Some(Metadata {
            country,
            number_type: type_name(number_type).into(),
            carrier,
            description,
        })
    }

    fn geographical_number(&self, number: &Number) -> Option<Number> {
        if number.country_code != 54 {
            return None;
        }
        let national = number.national.strip_prefix('9')?;
        number::parse(national, number::main_region(number))
    }

    fn prefix(&self, kind: &str, number: &Number, locale: &Locale) -> String {
        let key = if number.country_code == 1 {
            format!("1{}", &number.national[..3])
        } else {
            number.country_code.to_string()
        };
        let digits = format!("{}{}", number.country_code, number.national);
        let Ok(digits) = digits.parse() else {
            return String::new();
        };
        let lookup = |language: &str| {
            self.data
                .get(&format!("{kind}/{key}_{language}"))
                .map(|map| map.prefix(digits).to_owned())
                .unwrap_or_default()
        };
        let normalized = (locale.language == "zh"
            && matches!(locale.region.as_str(), "TW" | "HK" | "MO"))
        .then_some("zh_Hant");
        let mut candidates = Vec::new();
        if let Some(language) = normalized {
            candidates.push(language.to_owned());
        }
        if !locale.language.is_empty() {
            if !locale.region.is_empty() {
                candidates.push(format!("{}_{}", locale.language, locale.region));
            }
            candidates.push(locale.language.clone());
        }
        let description = candidates
            .iter()
            .find(|language| self.data.contains(&format!("{kind}/{key}_{language}")))
            .map(|language| lookup(language))
            .unwrap_or_default();
        if !description.is_empty() || matches!(locale.language.as_str(), "zh" | "ja" | "ko") {
            return description;
        }
        lookup("en")
    }

    fn country_name(&self, number: &Number, locale: &Locale) -> String {
        let mut valid = number::regions(number.country_code)
            .filter(|plan| plan.kind(&number.national).is_some());
        let Some(region) = valid.next().map(|plan| plan.region.as_str()) else {
            return String::new();
        };
        if valid.next().is_some() {
            return String::new();
        }
        if matches!(region, "001" | "ZZ") {
            return String::new();
        }
        for tag in locale.country_tags() {
            if let Some(map) = self.data.get(&format!("countries/{tag}")) {
                return map.country(region).to_owned();
            }
        }
        region.to_owned()
    }
}

fn type_name(kind: NumberType) -> &'static str {
    match kind {
        NumberType::FixedLine => "FIXED_LINE",
        NumberType::Mobile => "MOBILE",
        NumberType::FixedLineOrMobile => "FIXED_LINE_OR_MOBILE",
        NumberType::TollFree => "TOLL_FREE",
        NumberType::PremiumRate => "PREMIUM_RATE",
        NumberType::SharedCost => "SHARED_COST",
        NumberType::PersonalNumber => "PERSONAL_NUMBER",
        NumberType::Voip => "VOIP",
        NumberType::Pager => "PAGER",
        NumberType::Uan => "UAN",
        NumberType::Voicemail => "VOICEMAIL",
    }
}

#[cfg(test)]
#[path = "../../tests/unit/phone/mod.rs"]
mod tests;
