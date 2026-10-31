#[derive(Debug)]
pub(super) struct Locale {
    pub language: String,
    pub region: String,
    pub script: String,
}

impl Locale {
    pub fn parse(tag: &str) -> Self {
        let mut parts = tag.split('-');
        let language = parts.next().unwrap_or_default();
        if !(2..=8).contains(&language.len())
            || !letters(language)
            || language.eq_ignore_ascii_case("und")
        {
            return Self {
                language: String::new(),
                region: String::new(),
                script: String::new(),
            };
        }
        let language = match language.to_ascii_lowercase().as_str() {
            "iw" => "he".to_owned(),
            "in" => "id".to_owned(),
            "ji" => "yi".to_owned(),
            value => value.to_owned(),
        };
        let mut locale = Self {
            language,
            region: String::new(),
            script: String::new(),
        };
        for part in parts {
            if part.len() == 1 {
                break;
            }
            if part.len() == 4
                && letters(part)
                && locale.script.is_empty()
                && locale.region.is_empty()
            {
                locale.script = part.to_ascii_lowercase();
                locale.script[..1].make_ascii_uppercase();
            } else if locale.region.is_empty()
                && ((part.len() == 2 && letters(part))
                    || (part.len() == 3 && part.bytes().all(|c| c.is_ascii_digit())))
            {
                locale.region = part.to_ascii_uppercase();
            } else {
                break;
            }
        }
        locale
    }

    pub fn country_tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        if !self.language.is_empty() {
            if !self.script.is_empty() && !self.region.is_empty() {
                tags.push(format!("{}-{}-{}", self.language, self.script, self.region));
            }
            if !self.script.is_empty() {
                tags.push(format!("{}-{}", self.language, self.script));
            }
            if !self.region.is_empty() {
                tags.push(format!("{}-{}", self.language, self.region));
            }
            tags.push(self.language.clone());
        }
        tags.push("und".into());
        tags
    }
}

fn letters(value: &str) -> bool {
    value.bytes().all(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/phone/locale.rs"]
mod tests;
