type ParseResult<'a, T> = Result<(ParseInput<'a>, T), ParseError>;
type ParseInput<'a> = &'a [u8];
type ParseError = &'static str;
macro_rules ! any_of { ($ lt : lifetime , $ ($ parser : expr) ,+ $ (,) ?) => { move | input : crate :: zip :: windows_path :: ParseInput <$ lt >| { $ (if let Ok ((input , value)) = $ parser (input) { return Ok ((input , value)) ; }) + Err ("No parser succeeded") } } ; }
fn map<'a, T, U>(
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T>,
    f: impl Fn(T) -> U,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, U> {
    move |input: ParseInput| {
        let (input, value) = parser(input)?;
        Ok((input, f(value)))
    }
}
fn prefixed<'a, T1, T2>(
    mut prefix: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T1>,
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T2>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, T2> {
    move |input: ParseInput| {
        let (input, _) = prefix(input)?;
        let (input, value) = parser(input)?;
        Ok((input, value))
    }
}
fn suffixed<'a, T1, T2>(
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T1>,
    mut suffix: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T2>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, T1> {
    move |input: ParseInput| {
        let (input, value) = parser(input)?;
        let (input, _) = suffix(input)?;
        Ok((input, value))
    }
}
fn maybe<'a, T>(
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, Option<T>> {
    move |input: ParseInput| match parser(input) {
        Ok((input, value)) => Ok((input, Some(value))),
        Err(_) => Ok((input, None)),
    }
}
fn not<'a, T>(
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, ()> {
    move |input: ParseInput| match parser(input) {
        Ok(_) => Err("parser succeeded"),
        Err(_) => Ok((input, ())),
    }
}
fn peek<'a, T>(
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, T> {
    move |input: ParseInput| {
        let (_, value) = parser(input)?;
        Ok((input, value))
    }
}
fn zero_or_more<'a, T>(
    parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, Vec<T>> {
    let mut parser = maybe(one_or_more(parser));
    move |input: ParseInput| {
        let (input, results) = parser(input)?;
        Ok((input, results.unwrap_or_default()))
    }
}
fn take_until_byte(
    mut predicate: impl FnMut(u8) -> bool,
) -> impl FnMut(ParseInput) -> ParseResult<ParseInput> {
    move |input: ParseInput| {
        let (input, value) = match input.iter().enumerate().find(|(_, b)| predicate(**b)) {
            Some((0, _)) => (input, b"".as_slice()),
            Some((i, _)) => (&input[i..], &input[..i]),
            None => (b"".as_slice(), input),
        };
        Ok((input, value))
    }
}
fn take_until_byte_1(
    predicate: impl FnMut(u8) -> bool,
) -> impl FnMut(ParseInput) -> ParseResult<ParseInput> {
    let mut parser = take_until_byte(predicate);
    move |input: ParseInput| {
        let (input, value) = parser(input)?;
        if value.is_empty() {
            return Err("did not consume 1 byte");
        }
        Ok((input, value))
    }
}
fn take(cnt: usize) -> impl FnMut(ParseInput) -> ParseResult<ParseInput> {
    move |input: ParseInput| {
        if cnt == 0 {
            Err("take(cnt) cannot have cnt == 0")
        } else if cnt > input.len() {
            Err("take(cnt) not enough bytes")
        } else {
            Ok((&input[cnt..], &input[..cnt]))
        }
    }
}
fn bytes(bytes: &[u8]) -> impl FnMut(ParseInput) -> ParseResult<&[u8]> + '_ {
    move |input: ParseInput| {
        if input.is_empty() {
            return Err("Empty input");
        }
        if input.len() < bytes.len() {
            return Err("Not enough bytes");
        }
        if input.starts_with(bytes) {
            Ok((&input[bytes.len()..], &input[..bytes.len()]))
        } else {
            Err("Wrong bytes")
        }
    }
}
fn byte(byte: u8) -> impl FnMut(ParseInput) -> ParseResult<u8> {
    move |input: ParseInput| {
        if input.is_empty() {
            return Err("Empty input");
        }
        if input.starts_with(&[byte]) {
            Ok((&input[1..], byte))
        } else {
            Err("Wrong byte")
        }
    }
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum WindowsPrefix<'a> {
    Verbatim(&'a [u8]),
    VerbatimUNC(&'a [u8], &'a [u8]),
    VerbatimDisk(u8),
    DeviceNS(&'a [u8]),
    UNC(&'a [u8], &'a [u8]),
    Disk(u8),
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct WindowsPrefixComponent<'a> {
    raw: &'a [u8],
    parsed: WindowsPrefix<'a>,
}
impl WindowsPrefixComponent<'_> {
    fn len(&self) -> usize {
        self.raw.len()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum WindowsComponent<'a> {
    Prefix(WindowsPrefixComponent<'a>),
    RootDir,
    CurDir,
    ParentDir,
    Normal(&'a [u8]),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Component<'a> {
    Prefix,
    RootDir,
    CurDir,
    ParentDir,
    Normal(&'a str),
}
const SEPARATOR: char = '\\';
const ALT_SEPARATOR: char = '/';
const CURRENT_DIR: &[u8] = b".";
const PARENT_DIR: &[u8] = b"..";
pub(super) fn components(path: &str) -> impl Iterator<Item = Component<'_>> {
    let mut parser = Parser::new(path.as_bytes());
    std::iter::from_fn(move || {
        parser.next_front().ok().map(|c| match c {
            WindowsComponent::Prefix(_) => Component::Prefix,
            WindowsComponent::RootDir => Component::RootDir,
            WindowsComponent::CurDir => Component::CurDir,
            WindowsComponent::ParentDir => Component::ParentDir,
            WindowsComponent::Normal(bytes) => Component::Normal(
                std::str::from_utf8(bytes).expect("components slice UTF-8 at ASCII separators"),
            ),
        })
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Parser<'a> {
    input: &'a [u8],
    state: State,
    prefix: Option<WindowsPrefixComponent<'a>>,
    normalize: bool,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum State {
    AtBeginning,
    NotAtBeginning,
}
impl State {}
impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        let normalize = !input.starts_with(br"\\?\");
        let (_, prefix) = maybe(prefix_component)(input).unwrap();
        Self {
            input,
            state: State::AtBeginning,
            prefix,
            normalize,
        }
    }
    fn next_front(&mut self) -> Result<WindowsComponent<'a>, ParseError> {
        if let Some(prefix) = self.prefix.take() {
            self.input = &self.input[prefix.len()..];
            return Ok(WindowsComponent::Prefix(prefix));
        }
        let (input, component) = parse_front(self.state, self.normalize)(self.input)?;
        self.input = input;
        self.state = State::NotAtBeginning;
        Ok(component)
    }
}
fn parse_front(
    state: State,
    normalize: bool,
) -> impl FnMut(ParseInput) -> ParseResult<WindowsComponent> {
    move |input: ParseInput| {
        let root_dir = root_dir(normalize);
        let cur_dir = cur_dir(normalize);
        let filename = filename(normalize);
        let move_to_next = move_front_to_next(normalize);
        match state {
            State::AtBeginning => {
                suffixed(any_of ! ('_ , root_dir , cur_dir , filename), move_to_next)(input)
            }
            State::NotAtBeginning => suffixed(filename, move_to_next)(input),
        }
    }
}
fn root_dir(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<WindowsComponent> {
    move |input: ParseInput| {
        if input.is_empty() {
            Err("empty input")
        } else if !is_separator(input[0], normalize) {
            Err("not root dir")
        } else {
            Ok((&input[1..], WindowsComponent::RootDir))
        }
    }
}
fn filename(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<WindowsComponent> {
    let cur_dir = cur_dir(normalize);
    let parent_dir = parent_dir(normalize);
    let normal = normal(normalize);
    move |input: ParseInput| {
        if let Ok((input, value)) = parent_dir(input) {
            return Ok((input, value));
        }
        if !normalize {
            if let Ok((input, value)) = cur_dir(input) {
                return Ok((input, value));
            }
        }
        if let Ok((input, value)) = normal(input) {
            return Ok((input, value));
        }
        Err("invalid filename")
    }
}
fn cur_dir(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<WindowsComponent> {
    move |input: ParseInput| {
        let (input, _) = bytes(CURRENT_DIR)(input)?;
        if !input.is_empty() && !is_separator(input[0], normalize) {
            return Err("more non-separator bytes after parent dir");
        }
        Ok((input, WindowsComponent::CurDir))
    }
}
fn parent_dir(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<WindowsComponent> {
    move |input: ParseInput| {
        let (input, _) = bytes(PARENT_DIR)(input)?;
        if !input.is_empty() && !is_separator(input[0], normalize) {
            return Err("more non-separator bytes after parent dir");
        }
        Ok((input, WindowsComponent::ParentDir))
    }
}
fn normal(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<WindowsComponent> {
    move |input: ParseInput| {
        let (input, normal) = normal_bytes(normalize)(input)?;
        Ok((input, WindowsComponent::Normal(normal)))
    }
}
fn normal_bytes(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<ParseInput> {
    move |input: ParseInput| {
        let (input, normal) = take_until_byte_1(|b| is_separator(b, normalize))(input)?;
        Ok((input, normal))
    }
}
fn move_front_to_next(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<()> {
    move |input: ParseInput| {
        if normalize {
            let parser = zero_or_more(
                any_of ! ('_ , separator (normalize) , map (cur_dir (normalize) , | _ | ())),
            );
            map(parser, |_| ())(input)
        } else {
            map(zero_or_more(separator(normalize)), |_| ())(input)
        }
    }
}
fn is_separator(b: u8, normalize: bool) -> bool {
    b == SEPARATOR as u8 || (normalize && b == ALT_SEPARATOR as u8)
}
fn separator(normalize: bool) -> impl Fn(ParseInput) -> ParseResult<()> {
    move |input: ParseInput| {
        if input.starts_with(&[SEPARATOR as u8])
            || (normalize && input.starts_with(&[ALT_SEPARATOR as u8]))
        {
            Ok((&input[1..], ()))
        } else {
            Err("not a separator")
        }
    }
}
fn prefix_component(input: ParseInput) -> ParseResult<WindowsPrefixComponent> {
    let (new_input, parsed) = prefix(input)?;
    Ok((
        new_input,
        WindowsPrefixComponent {
            raw: &input[..(input.len() - new_input.len())],
            parsed,
        },
    ))
}
fn prefix<'a>(input: ParseInput<'a>) -> ParseResult<WindowsPrefix> {
    any_of ! ('a , prefix_verbatim_unc , prefix_verbatim_disk , prefix_verbatim , prefix_device_ns , prefix_unc , prefix_disk ,)(
        input,
    )
}
fn prefix_verbatim_unc(input: ParseInput) -> ParseResult<WindowsPrefix> {
    let normalize = !input.starts_with(br"\\?\");
    let (input, _) = verbatim(input)?;
    let (input, _) = bytes(b"UNC")(input)?;
    let (input, _) = separator(normalize)(input)?;
    let (input, server) = normal_bytes(normalize)(input)?;
    let (input, _) = maybe(separator(normalize))(input)?;
    let (input, maybe_share) = maybe(normal_bytes(normalize))(input)?;
    Ok((
        input,
        WindowsPrefix::VerbatimUNC(server, maybe_share.unwrap_or(b"")),
    ))
}
fn prefix_verbatim<'a>(input: ParseInput<'a>) -> ParseResult<WindowsPrefix> {
    let (input, _) = not(prefix_verbatim_disk)(input)?;
    let (input, _) = not(prefix_verbatim_unc)(input)?;
    let normalized = !input.starts_with(br"\\?\");
    let (input, _) = verbatim(input)?;
    let (input, value) = any_of ! ('a , normal_bytes (normalized) , map (peek (separator (normalized)) , | _ | b""))(
        input,
    )?;
    Ok((input, WindowsPrefix::Verbatim(value)))
}
fn prefix_verbatim_disk(input: ParseInput) -> ParseResult<WindowsPrefix> {
    map(prefixed(verbatim, disk_byte), WindowsPrefix::VerbatimDisk)(input)
}
fn verbatim(input: ParseInput) -> ParseResult<()> {
    let (input, _) = separator(true)(input)?;
    let (input, _) = separator(true)(input)?;
    let (input, _) = byte(b'?')(input)?;
    let (input, _) = separator(true)(input)?;
    Ok((input, ()))
}
fn prefix_device_ns(input: ParseInput) -> ParseResult<WindowsPrefix> {
    let (input, _) = separator(true)(input)?;
    let (input, _) = separator(true)(input)?;
    let (input, _) = byte(b'.')(input)?;
    let (input, _) = separator(true)(input)?;
    map(normal_bytes(true), WindowsPrefix::DeviceNS)(input)
}
fn prefix_unc(input: ParseInput) -> ParseResult<WindowsPrefix> {
    let (input, _) = separator(true)(input)?;
    let (input, _) = separator(true)(input)?;
    let (input, server) = normal_bytes(true)(input)?;
    let (input, _) = maybe(separator(true))(input)?;
    let (input, maybe_share) = maybe(normal_bytes(true))(input)?;
    Ok((
        input,
        WindowsPrefix::UNC(server, maybe_share.unwrap_or(b"")),
    ))
}
fn prefix_disk(input: ParseInput) -> ParseResult<WindowsPrefix> {
    map(disk_byte, WindowsPrefix::Disk)(input)
}
fn disk_byte(input: ParseInput) -> ParseResult<u8> {
    let (input, drive_letter) = drive_letter(input)?;
    let (input, _) = byte(b':')(input)?;
    Ok((input, drive_letter))
}
fn drive_letter(input: ParseInput) -> ParseResult<u8> {
    let (input, drive_letter) = take(1)(input)?;
    if !(drive_letter[0] as char).is_alphabetic() {
        return Err("drive not alphabetic");
    }
    Ok((input, drive_letter[0].to_ascii_uppercase()))
}

/// Takes while the parser returns true, returning a collection of parser results, or failing if
/// the parser did not succeed at least once
fn one_or_more<'a, T>(
    mut parser: impl FnMut(ParseInput<'a>) -> ParseResult<'a, T>,
) -> impl FnMut(ParseInput<'a>) -> ParseResult<'a, Vec<T>> {
    move |input: ParseInput| {
        let mut results = Vec::new();
        let mut next = Some(input);
        while let Some(input) = next.take() {
            match parser(input) {
                Ok((input, value)) => {
                    next = Some(input);
                    results.push(value);
                }
                Err(_) => {
                    next = Some(input);
                    break;
                }
            }
        }

        if results.is_empty() {
            return Err("Parser failed to suceed once");
        }

        Ok((next.unwrap(), results))
    }
}
