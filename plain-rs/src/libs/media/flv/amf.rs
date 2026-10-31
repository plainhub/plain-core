use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) enum Scalar<'a> {
    Number(f64),
    Text(&'a str),
    Other,
}
struct Reader<'a> {
    data: &'a [u8],
    nodes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (bytes, rest) = self.data.split_at_checked(count)?;
        self.data = rest;
        Some(bytes)
    }
    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn short_text(&mut self) -> Option<&'a [u8]> {
        let length = usize::from(u16::from_be_bytes(self.take(2)?.try_into().ok()?));
        self.take(length)
    }
    fn long_text(&mut self) -> Option<&'a [u8]> {
        let length = usize::try_from(u32::from_be_bytes(self.take(4)?.try_into().ok()?)).ok()?;
        self.take(length)
    }
    fn object(&mut self, depth: u8, mut field: impl FnMut(&'a [u8], Scalar<'a>)) -> Option<()> {
        loop {
            let key = self.short_text()?;
            if key.is_empty() && self.data.first() == Some(&9) {
                self.byte()?;
                return Some(());
            }
            field(key, self.value(depth + 1)?);
        }
    }
    fn value(&mut self, depth: u8) -> Option<Scalar<'a>> {
        self.nodes = self.nodes.checked_sub(1)?;
        if depth > 8 {
            return None;
        }
        match self.byte()? {
            0 => Some(Scalar::Number(f64::from_be_bytes(
                self.take(8)?.try_into().ok()?,
            ))),
            1 => {
                self.take(1)?;
                Some(Scalar::Other)
            }
            2 => Some(
                std::str::from_utf8(self.short_text()?)
                    .ok()
                    .map(Scalar::Text)
                    .unwrap_or(Scalar::Other),
            ),
            3 => {
                self.object(depth, |_, _| {})?;
                Some(Scalar::Other)
            }
            5 | 6 | 13 => Some(Scalar::Other),
            7 => {
                self.take(2)?;
                Some(Scalar::Other)
            }
            8 => {
                self.take(4)?;
                self.object(depth, |_, _| {})?;
                Some(Scalar::Other)
            }
            10 => {
                let count =
                    usize::try_from(u32::from_be_bytes(self.take(4)?.try_into().ok()?)).ok()?;
                if count > self.nodes {
                    return None;
                }
                for _ in 0..count {
                    self.value(depth + 1)?;
                }
                Some(Scalar::Other)
            }
            11 => {
                self.take(10)?;
                Some(Scalar::Other)
            }
            12 | 15 => Some(
                std::str::from_utf8(self.long_text()?)
                    .ok()
                    .map(Scalar::Text)
                    .unwrap_or(Scalar::Other),
            ),
            16 => {
                self.short_text()?;
                self.object(depth, |_, _| {})?;
                Some(Scalar::Other)
            }
            _ => None,
        }
    }
}
pub(super) fn metadata(data: &[u8]) -> Option<HashMap<&str, Scalar<'_>>> {
    let mut reader = Reader {
        data,
        nodes: 100_000,
    };
    let event = reader.value(0)?;
    if matches!(event, Scalar::Text("@setDataFrame"))
        && !matches!(reader.value(0)?, Scalar::Text("onMetaData"))
    {
        return None;
    }
    if !matches!(event, Scalar::Text("onMetaData" | "@setDataFrame")) {
        return None;
    }
    match reader.byte()? {
        3 => {}
        8 => {
            reader.take(4)?;
        }
        _ => return None,
    }
    let mut fields = HashMap::new();
    reader.object(0, |key, value| {
        if let Ok(key) = std::str::from_utf8(key)
            && matches!(
                key,
                "duration"
                    | "width"
                    | "height"
                    | "framerate"
                    | "videodatarate"
                    | "audiodatarate"
                    | "title"
                    | "artist"
                    | "album"
                    | "genre"
                    | "author"
                    | "composer"
                    | "creationdate"
            )
        {
            fields.insert(key, value);
        }
    })?;
    Some(fields)
}
