use super::codes::{self, Mode};
use super::{ByteReader, Color, Transitions};

fn read_run<E>(
    color: Color,
    reader: &mut ByteReader<impl Iterator<Item = Result<u8, E>>>,
) -> Option<u16> {
    let mut total = 0u16;
    loop {
        let n = codes::run(color, reader)?;
        total = total.checked_add(n)?;
        if n < 64 {
            return Some(total);
        }
    }
}

pub fn pels(line: &[u16], width: u16) -> impl Iterator<Item = Color> + '_ {
    use std::iter::repeat;
    let mut color = Color::White;
    let mut last = 0;
    let pad_color = if line.len() & 1 == 1 { !color } else { color };
    line.iter()
        .flat_map(move |&p| {
            let c = color;
            color = !color;
            let n = p.saturating_sub(last);
            last = p;
            repeat(c).take(n as usize)
        })
        .chain(repeat(pad_color))
        .take(width as usize)
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum DecodeStatus {
    Incomplete,
    End,
}

#[derive(Debug)]
pub enum DecodeError<E> {
    Reader(E),
    Invalid,
    Unsupported,
}
impl<E> std::fmt::Display for DecodeError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Decode Error")
    }
}
impl<E: std::error::Error> std::error::Error for DecodeError<E> {}

pub struct Group4Decoder<R> {
    reader: ByteReader<R>,
    reference: Vec<u16>,
    current: Vec<u16>,
    width: u16,
}
impl<E, R: Iterator<Item = Result<u8, E>>> Group4Decoder<R> {
    pub fn new(reader: R, width: u16) -> Result<Self, E> {
        Ok(Group4Decoder {
            reader: ByteReader::new(reader)?,
            reference: Vec::new(),
            current: Vec::new(),
            width,
        })
    }
    pub fn advance(&mut self) -> Result<DecodeStatus, DecodeError<E>> {
        let mut transitions = Transitions::new(&self.reference);
        let mut a0 = 0;
        let mut color = Color::White;
        let mut start_of_row = true;

        loop {
            let mode = match codes::mode(&mut self.reader) {
                Some(mode) => mode,
                None => return Err(DecodeError::Invalid),
            };

            match mode {
                Mode::Pass => {
                    if start_of_row && color == Color::White {
                        transitions.pos += 1;
                    } else {
                        transitions
                            .next_color(a0, !color, false)
                            .ok_or(DecodeError::Invalid)?;
                    }
                    if let Some(b2) = transitions.next() {
                        a0 = b2;
                    }
                }
                Mode::Vertical(delta) => {
                    let b1 = transitions
                        .next_color(a0, !color, start_of_row)
                        .unwrap_or(self.width);
                    let Ok(a1) = u16::try_from(i32::from(b1) + i32::from(delta)) else {
                        break;
                    };
                    if a1 >= self.width {
                        break;
                    }
                    self.current.push(a1);
                    color = !color;
                    a0 = a1;
                    if delta < 0 {
                        transitions.seek_back(a0);
                    }
                }
                Mode::Horizontal => {
                    let a0a1 = read_run(color, &mut self.reader).ok_or(DecodeError::Invalid)?;
                    let a1a2 = read_run(!color, &mut self.reader).ok_or(DecodeError::Invalid)?;
                    let a1 = a0.checked_add(a0a1).ok_or(DecodeError::Invalid)?;
                    let a2 = a1.checked_add(a1a2).ok_or(DecodeError::Invalid)?;

                    self.current.push(a1);
                    if a2 >= self.width {
                        break;
                    }
                    self.current.push(a2);
                    a0 = a2;
                }
                Mode::Extension => {
                    if self.reader.valid < 3 {
                        return Err(DecodeError::Invalid);
                    }
                    self.reader.consume(3).map_err(DecodeError::Reader)?;
                    return Err(DecodeError::Unsupported);
                }
                Mode::End => return Ok(DecodeStatus::End),
            }
            start_of_row = false;

            if a0 >= self.width {
                break;
            }
        }

        std::mem::swap(&mut self.reference, &mut self.current);
        self.current.clear();

        Ok(DecodeStatus::Incomplete)
    }

    pub fn transition(&self) -> &[u16] {
        &self.reference
    }
}
