pub(crate) fn decode(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut position = 0;
    while position < input.len() {
        let Some(relative) = input[position..].find('&') else {
            output.push_str(&input[position..]);
            break;
        };
        let start = position + relative;
        output.push_str(&input[position..start]);
        let bytes = input.as_bytes();
        let mut end = start + 1;
        let radix = if bytes.get(end) == Some(&b'#') {
            end += 1;
            if bytes.get(end) == Some(&b'x') {
                end += 1;
                16
            } else {
                10
            }
        } else {
            0
        };
        let name_start = end;
        while let Some(&c) = bytes.get(end) {
            let valid = match radix {
                16 => c.is_ascii_hexdigit(),
                10 => c.is_ascii_digit(),
                _ if end == name_start => c.is_ascii_alphabetic(),
                _ => c.is_ascii_alphanumeric(),
            };
            if !valid {
                break;
            }
            end += 1;
        }
        if end == name_start || bytes.get(end) != Some(&b';') {
            output.push('&');
            position = start + 1;
            continue;
        }
        let name = &input[name_start..end];
        let scalar = (radix != 0)
            .then(|| {
                u32::from_str_radix(name, radix)
                    .ok()
                    .and_then(char::from_u32)
            })
            .flatten();
        if let Some(c) = scalar {
            output.push(c);
        } else if let Some(value) = (radix == 0).then(|| named(name)).flatten() {
            output.push_str(value);
        } else {
            output.push_str(&input[start..=end]);
        }
        position = end + 1;
    }
    output
}

fn named(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        "copy" => "\u{00A9}",
        "reg" => "\u{00AE}",
        "trade" => "\u{2122}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "hellip" => "\u{2026}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "ldquo" => "\u{201C}",
        "rdquo" => "\u{201D}",
        "laquo" => "\u{00AB}",
        "raquo" => "\u{00BB}",
        "deg" => "\u{00B0}",
        "plusmn" => "\u{00B1}",
        "times" => "\u{00D7}",
        "divide" => "\u{00F7}",
        "frac12" => "\u{00BD}",
        "frac14" => "\u{00BC}",
        "frac34" => "\u{00BE}",
        "sup2" => "\u{00B2}",
        "sup3" => "\u{00B3}",
        "micro" => "\u{00B5}",
        "para" => "\u{00B6}",
        "middot" => "\u{00B7}",
        "cent" => "\u{00A2}",
        "pound" => "\u{00A3}",
        "yen" => "\u{00A5}",
        "euro" => "\u{20AC}",
        "sect" => "\u{00A7}",
        "bull" => "\u{2022}",
        "dagger" => "\u{2020}",
        "Dagger" => "\u{2021}",
        "permil" => "\u{2030}",
        "prime" => "\u{2032}",
        "Prime" => "\u{2033}",
        "infin" => "\u{221E}",
        "ne" => "\u{2260}",
        "le" => "\u{2264}",
        "ge" => "\u{2265}",
        "larr" => "\u{2190}",
        "uarr" => "\u{2191}",
        "rarr" => "\u{2192}",
        "darr" => "\u{2193}",
        "harr" => "\u{2194}",
        "spades" => "\u{2660}",
        "clubs" => "\u{2663}",
        "hearts" => "\u{2665}",
        "diams" => "\u{2666}",
        "alpha" => "\u{03B1}",
        "beta" => "\u{03B2}",
        "gamma" => "\u{03B3}",
        "delta" => "\u{03B4}",
        "epsilon" => "\u{03B5}",
        "pi" => "\u{03C0}",
        "omega" => "\u{03C9}",
        "Alpha" => "\u{0391}",
        "Beta" => "\u{0392}",
        "Gamma" => "\u{0393}",
        "Delta" => "\u{0394}",
        "Omega" => "\u{03A9}",
        _ => return None,
    })
}
