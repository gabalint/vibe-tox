//! Reading HL7 v2 text, for the order reader and the acknowledgement reader (ticket #4555;
//! ADR 0007 §7): the separators MSH-1 and MSH-2 declare, segments with their fields numbered
//! as HL7 numbers them, and component values unescaped. Pure.

/// The separators of one message.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Separators {
    pub(crate) field: char,
    pub(crate) component: char,
    pub(crate) repetition: char,
    pub(crate) escape: char,
    pub(crate) subcomponent: char,
}

/// One segment's fields, numbered as HL7 numbers them (MSH-1 is the field separator).
pub(crate) struct Segment<'a> {
    pub(crate) name: &'a str,
    fields: Vec<&'a str>,
}

impl Segment<'_> {
    pub(crate) fn field(&self, number: usize) -> &str {
        self.fields.get(number).copied().unwrap_or("")
    }
}

/// The message's text without MLLP framing or a byte order mark; `None` if not UTF-8.
pub(crate) fn message_text(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    Some(text.trim_matches(|c| c == '\u{b}' || c == '\u{1c}' || c == '\u{feff}'))
}

/// MSH-1 and MSH-2; `None` without an MSH segment first, or with unusable separators.
pub(crate) fn separators(text: &str) -> Option<Separators> {
    let mut chars = text.strip_prefix("MSH")?.chars();
    let separators = Separators {
        field: chars.next()?,
        component: chars.next()?,
        repetition: chars.next()?,
        escape: chars.next()?,
        subcomponent: chars.next()?,
    };
    let all = [
        separators.field,
        separators.component,
        separators.repetition,
        separators.escape,
        separators.subcomponent,
    ];
    let distinct = all
        .iter()
        .enumerate()
        .all(|(index, c)| !all[..index].contains(c));
    (distinct && all.iter().all(char::is_ascii_punctuation)).then_some(separators)
}

/// The message's segments, in order; MSH first, as [`separators`] found it.
pub(crate) fn segments(text: &str, separators: Separators) -> Vec<Segment<'_>> {
    text.split(['\r', '\n'])
        .filter(|line| !line.trim().is_empty())
        .map(|line| segment(line, separators))
        .collect()
}

fn segment(line: &str, separators: Separators) -> Segment<'_> {
    let mut parts: Vec<&str> = line.split(separators.field).collect();
    let name = parts.first().copied().unwrap_or("");
    if name == "MSH" {
        // MSH-1 is the field separator itself, so MSH-2 is the second part.
        parts.insert(1, "");
    }
    Segment {
        name,
        fields: parts,
    }
}

pub(crate) fn find<'a, 'b>(segments: &'b [Segment<'a>], name: &str) -> Option<&'b Segment<'a>> {
    segments.iter().find(|segment| segment.name == name)
}

/// Reads values with one message's separators.
pub(crate) struct Reader {
    pub(crate) separators: Separators,
}

impl Reader {
    /// The first repetition's component `number` (1-based), unescaped and trimmed.
    pub(crate) fn component(&self, field: &str, number: usize) -> String {
        let first = field.split(self.separators.repetition).next().unwrap_or("");
        let component = first.split(self.separators.component).nth(number - 1);
        self.text(component.unwrap_or(""))
    }

    /// The first subcomponent of `value`, unescaped and trimmed.
    pub(crate) fn text(&self, value: &str) -> String {
        let first = value
            .split(self.separators.subcomponent)
            .next()
            .unwrap_or("");
        self.unescape(first).trim().to_owned()
    }

    pub(crate) fn some(&self, value: &str) -> Option<String> {
        (!value.is_empty()).then(|| value.to_owned())
    }

    /// The standard escapes; any other escape sequence is kept as written.
    fn unescape(&self, value: &str) -> String {
        let escape = self.separators.escape;
        let mut out = String::with_capacity(value.len());
        let mut rest = value;
        while let Some(start) = rest.find(escape) {
            out.push_str(&rest[..start]);
            let after = &rest[start + escape.len_utf8()..];
            let Some(end) = after.find(escape) else {
                out.push_str(&rest[start..]);
                return out;
            };
            match &after[..end] {
                "F" => out.push(self.separators.field),
                "S" => out.push(self.separators.component),
                "R" => out.push(self.separators.repetition),
                "E" => out.push(escape),
                "T" => out.push(self.separators.subcomponent),
                other => {
                    out.push(escape);
                    out.push_str(other);
                    out.push(escape);
                }
            }
            rest = &after[end + escape.len_utf8()..];
        }
        out.push_str(rest);
        out
    }
}
