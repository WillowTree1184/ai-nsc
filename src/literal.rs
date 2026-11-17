//! Literal decoding.
//!
//! The lexer only validates the *surface* form of literals and keeps their raw
//! text. This module turns that text into values, applying the rules of §2.7:
//! digit separators are erased, bases are honoured, escape sequences are
//! resolved, encoding tags (`:ascii`, `:utf8`, ...) select the representation,
//! and multi-line normal strings drop the line terminator plus the whitespace
//! that follows it.
//!
//! String and character literals become byte arrays / scalar values; their
//! types are `array(L/N) cardinal(N)` and `cardinal(N)` (§2.7.4, §2.7.5), which
//! the semantic analysis layer derives using [`Encoding`].

/// A character encoding a literal may be tagged with (§2.7.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    /// ASCII, 1-byte units.
    Ascii,
    /// UTF-8, 1-byte string units and a 4-byte character type.
    Utf8,
    /// UTF-16, 2-byte units (a single character may need 4 bytes).
    Utf16,
    /// UTF-32, 4-byte units.
    Utf32,
    /// UCS-2, 2-byte units, only scalar values up to U+FFFF.
    Ucs2,
    /// UCS-4, 4-byte units.
    Ucs4,
}

impl Encoding {
    /// Parses an encoding tag (`ascii`, `utf8`, `utf16`, `utf32`, `ucs2`,
    /// `ucs4`).
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "ascii" => Encoding::Ascii,
            "utf8" => Encoding::Utf8,
            "utf16" => Encoding::Utf16,
            "utf32" => Encoding::Utf32,
            "ucs2" => Encoding::Ucs2,
            "ucs4" => Encoding::Ucs4,
            _ => return None,
        })
    }

    /// The tag spelling of this encoding.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Encoding::Ascii => "ascii",
            Encoding::Utf8 => "utf8",
            Encoding::Utf16 => "utf16",
            Encoding::Utf32 => "utf32",
            Encoding::Ucs2 => "ucs2",
            Encoding::Ucs4 => "ucs4",
        }
    }

    /// Whether a character literal must carry an explicit tag (§2.7.6 table).
    #[must_use]
    pub const fn requires_explicit_tag_for_character(self) -> bool {
        match self {
            Encoding::Ascii | Encoding::Utf8 => false,
            Encoding::Utf16 | Encoding::Utf32 | Encoding::Ucs2 | Encoding::Ucs4 => true,
        }
    }

    /// Whether a string literal must carry an explicit tag (§2.7.6 table).
    #[must_use]
    pub const fn requires_explicit_tag_for_string(self) -> bool {
        match self {
            Encoding::Utf8 => false,
            Encoding::Ascii
            | Encoding::Utf16
            | Encoding::Utf32
            | Encoding::Ucs2
            | Encoding::Ucs4 => true,
        }
    }

    /// The `N` of a `cardinal(N)` character type for `character` in this
    /// encoding (§2.7.6 table).
    #[must_use]
    pub fn character_unit_size(self, character: char) -> u32 {
        match self {
            Encoding::Ascii => 1,
            Encoding::Utf8 => 4,
            Encoding::Utf16 => {
                if u32::from(character) <= 0xFFFF {
                    2
                } else {
                    4
                }
            }
            Encoding::Utf32 | Encoding::Ucs4 => 4,
            Encoding::Ucs2 => 2,
        }
    }

    /// The `N` of the `array(L / N) cardinal(N)` string type (§2.7.6 table).
    #[must_use]
    pub const fn string_unit_size(self) -> u32 {
        match self {
            Encoding::Ascii | Encoding::Utf8 => 1,
            Encoding::Utf16 | Encoding::Ucs2 => 2,
            Encoding::Utf32 | Encoding::Ucs4 => 4,
        }
    }

    /// Encodes one scalar value as bytes in this encoding.
    ///
    /// # Errors
    ///
    /// Returns an error when the scalar value cannot be represented.
    pub fn encode_character(self, character: char, bytes: &mut Vec<u8>) -> Result<(), String> {
        match self {
            Encoding::Ascii => {
                if u32::from(character) > 0x7F {
                    return Err(format!(
                        "character '{}' is outside the ASCII range",
                        character
                    ));
                }
                bytes.push(character as u8);
            }
            Encoding::Utf8 => {
                let mut buffer = [0u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
            Encoding::Utf16 => {
                for unit in character.encode_utf16(&mut [0u16; 2]).iter() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
            }
            Encoding::Ucs2 => {
                let value = u32::from(character);
                if value > 0xFFFF {
                    return Err(format!(
                        "character U+{value:04X} cannot be represented in UCS-2"
                    ));
                }
                bytes.extend_from_slice(&(value as u16).to_le_bytes());
            }
            Encoding::Utf32 | Encoding::Ucs4 => {
                bytes.extend_from_slice(&u32::from(character).to_le_bytes());
            }
        }
        Ok(())
    }
}

/// A decoded character literal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DecodedCharacter {
    /// The scalar value of the literal.
    pub character: char,
    /// Explicit encoding tag, if one was written.
    pub encoding: Option<Encoding>,
    /// `N` of the resulting `cardinal(N)` type.
    pub unit_size: u32,
}

/// A decoded string literal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DecodedString {
    /// Encoded bytes (no NUL terminator, §2.7.5).
    pub bytes: Vec<u8>,
    /// Encoding used to produce the bytes.
    pub encoding: Encoding,
    /// `N` of the `array(L / N) cardinal(N)` type.
    pub unit_size: u32,
}

impl DecodedString {
    /// `L`: the encoded byte length.
    #[must_use]
    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    /// `L / N`: the number of elements of the array type.
    #[must_use]
    pub fn element_count(&self) -> u32 {
        self.bytes.len() as u32 / self.unit_size
    }
}

/// Decodes an integer literal (`42`, `0xFF`, `0b1010`, with `_` separators).
///
/// # Errors
///
/// Returns a message for malformed literals and values that do not fit in 128
/// bits.
pub fn decode_integer(raw: &str) -> Result<u128, String> {
    let cleaned: String = raw.chars().filter(|character| *character != '_').collect();
    let (digits, radix) = if let Some(rest) = cleaned.strip_prefix("0x") {
        (rest, 16)
    } else if let Some(rest) = cleaned.strip_prefix("0b") {
        (rest, 2)
    } else {
        (cleaned.as_str(), 10)
    };
    if digits.is_empty() {
        return Err("integer literal requires at least one digit".to_string());
    }
    u128::from_str_radix(digits, radix).map_err(|_| format!("invalid integer literal '{raw}'"))
}

/// Decodes a float literal (`3.14`, `1.5e10`) to an `f64`.
///
/// # Errors
///
/// Returns a message for malformed literals.
pub fn decode_float(raw: &str) -> Result<f64, String> {
    let cleaned: String = raw.chars().filter(|character| *character != '_').collect();
    cleaned
        .parse::<f64>()
        .map_err(|_| format!("invalid float literal '{raw}'"))
}

/// Decodes a character literal, including its optional encoding tag.
///
/// # Errors
///
/// Returns a message for malformed literals and encoding violations.
pub fn decode_character(raw: &str) -> Result<DecodedCharacter, String> {
    let (body, encoding) = split_encoding_tag(raw)?;
    if !body.starts_with('\'') || !body.ends_with('\'') || body.len() < 3 {
        return Err(format!("malformed character literal '{raw}'"));
    }
    let inner = &body[1..body.len() - 1];
    let character = decode_character_content(inner, encoding)?;
    if let Some(encoding) = encoding {
        // An explicit tag must be able to represent the character (§2.7.6).
        let mut probe = Vec::new();
        encoding.encode_character(character, &mut probe)?;
    }
    let unit_size = encoding.map_or(4, |encoding| encoding.character_unit_size(character));
    Ok(DecodedCharacter {
        character,
        encoding,
        unit_size,
    })
}

/// Decodes a string literal, including its optional encoding tag.
///
/// # Errors
///
/// Returns a message for malformed literals and encoding violations.
pub fn decode_string(raw: &str) -> Result<DecodedString, String> {
    let (body, tag) = split_encoding_tag(raw)?;
    let encoding = tag.unwrap_or(Encoding::Utf8);

    let (content, verbatim) = if let Some(rest) = body.strip_prefix("@\"") {
        (rest, true)
    } else if let Some(rest) = body.strip_prefix('"') {
        (rest, false)
    } else {
        return Err(format!("malformed string literal '{raw}'"));
    };
    if !content.ends_with('"') {
        return Err(format!("malformed string literal '{raw}'"));
    }
    let content = &content[..content.len() - 1];

    let mut bytes = Vec::new();
    let mut index = 0;
    let characters: Vec<char> = content.chars().collect();
    while index < characters.len() {
        let character = characters[index];
        if verbatim {
            if character == '"' && characters.get(index + 1) == Some(&'"') {
                encoding.encode_character('"', &mut bytes)?;
                index += 2;
                continue;
            }
            encoding.encode_character(character, &mut bytes)?;
            index += 1;
            continue;
        }

        match character {
            '\\' => {
                let (decoded, consumed) = decode_escape(&characters[index + 1..], false, encoding)?;
                for value in decoded {
                    encoding.encode_character(value, &mut bytes)?;
                }
                index += 1 + consumed;
            }
            '\n' | '\r' => {
                // A line terminator inside a normal string is not content; the
                // whitespace that follows it is not content either (§2.7.5).
                index += 1;
                while index < characters.len() && characters[index].is_whitespace() {
                    index += 1;
                }
            }
            _ => {
                encoding.encode_character(character, &mut bytes)?;
                index += 1;
            }
        }
    }

    Ok(DecodedString {
        bytes,
        encoding,
        unit_size: encoding.string_unit_size(),
    })
}

/// Splits a trailing `:tag` off a literal's raw text.
fn split_encoding_tag(raw: &str) -> Result<(&str, Option<Encoding>), String> {
    if let Some(colon) = raw.rfind(':') {
        let tag = &raw[colon + 1..];
        if let Some(encoding) = Encoding::from_tag(tag) {
            return Ok((&raw[..colon], Some(encoding)));
        }
    }
    Ok((raw, None))
}

/// Decodes the single character (or escape sequence) of a character literal.
fn decode_character_content(content: &str, encoding: Option<Encoding>) -> Result<char, String> {
    let characters: Vec<char> = content.chars().collect();
    if characters.is_empty() {
        return Err("character literal must contain exactly one character".to_string());
    }
    let (decoded, consumed) = if characters[0] == '\\' {
        let (decoded, consumed) =
            decode_escape(&characters[1..], true, encoding.unwrap_or(Encoding::Utf8))?;
        (decoded, consumed + 1)
    } else {
        (vec![characters[0]], 1)
    };
    if consumed != characters.len() {
        return Err("character literal must contain exactly one character".to_string());
    }
    if decoded.len() != 1 {
        return Err("escape sequence in a character literal must denote one character".to_string());
    }
    Ok(decoded[0])
}

/// Decodes one escape sequence. Returns the produced characters and how many
/// input characters were consumed (after the backslash).
fn decode_escape(
    rest: &[char],
    in_character: bool,
    encoding: Encoding,
) -> Result<(Vec<char>, usize), String> {
    let Some(&marker) = rest.first() else {
        return Err("unterminated escape sequence".to_string());
    };
    let simple = match marker {
        'a' => Some('\u{0007}'),
        'b' => Some('\u{0008}'),
        'f' => Some('\u{000C}'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'v' => Some('\u{000B}'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '0' => Some('\0'),
        _ => None,
    };
    if let Some(character) = simple {
        return Ok((vec![character], 1));
    }

    let digits = match marker {
        'x' => 2,
        'u' => 4,
        'U' => 8,
        _ => return Err(format!("invalid escape sequence '\\{marker}'")),
    };
    if rest.len() < 1 + digits {
        return Err("truncated escape sequence".to_string());
    }
    let mut value: u32 = 0;
    for index in 0..digits {
        let digit = rest[1 + index]
            .to_digit(16)
            .ok_or_else(|| "invalid hexadecimal digit in escape sequence".to_string())?;
        value = value * 16 + digit;
    }

    if marker == 'x' {
        // A byte escape: only meaningful for byte-oriented encodings when used
        // in a string, and as a scalar value in a character literal.
        let byte =
            u8::try_from(value).map_err(|_| "byte escape sequence out of range".to_string())?;
        if in_character {
            let character =
                char::from_u32(u32::from(byte)).ok_or_else(|| "invalid character".to_string())?;
            return Ok((vec![character], 1 + digits));
        }
        if encoding == Encoding::Ascii && byte > 0x7F {
            return Err("byte is outside the ASCII range".to_string());
        }
        return Ok((vec![char::from(byte)], 1 + digits));
    }

    let character = char::from_u32(value)
        .ok_or_else(|| "escape sequence denotes an invalid Unicode scalar value".to_string())?;
    Ok((vec![character], 1 + digits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers() {
        assert_eq!(decode_integer("42"), Ok(42));
        assert_eq!(decode_integer("1_000_000"), Ok(1_000_000));
        assert_eq!(decode_integer("0xFF"), Ok(255));
        assert_eq!(decode_integer("0b1010"), Ok(10));
        assert!(decode_integer("0x").is_err());
        assert!(decode_integer("0b2010").is_err());
    }

    #[test]
    fn floats() {
        assert!((decode_float("2.5").unwrap() - 2.5).abs() < f64::EPSILON);
        assert!((decode_float("1.5e10").unwrap() - 1.5e10).abs() < 1.0);
        assert!((decode_float("2.5e-3").unwrap() - 2.5e-3).abs() < 1e-12);
    }

    #[test]
    fn characters() {
        let decoded = decode_character("'a'").unwrap();
        assert_eq!(decoded.character, 'a');
        assert_eq!(decoded.encoding, None);
        assert_eq!(decoded.unit_size, 4);

        let decoded = decode_character("'中'").unwrap();
        assert_eq!(decoded.character, '中');

        let decoded = decode_character(r"'\n'").unwrap();
        assert_eq!(decoded.character, '\n');

        let decoded = decode_character("'a':ascii").unwrap();
        assert_eq!(decoded.encoding, Some(Encoding::Ascii));
        assert_eq!(decoded.unit_size, 1);

        let decoded = decode_character("'a':utf16").unwrap();
        assert_eq!(decoded.unit_size, 2);

        assert!(decode_character("'中':ascii").is_err());
        assert!(decode_character("'ab'").is_err());
        assert!(decode_character("''").is_err());
    }

    #[test]
    fn strings() {
        let decoded = decode_string(r#""Hello""#).unwrap();
        assert_eq!(decoded.bytes, b"Hello");
        assert_eq!(decoded.encoding, Encoding::Utf8);
        assert_eq!(decoded.element_count(), 5);

        let decoded = decode_string(r#""a\nb""#).unwrap();
        assert_eq!(decoded.bytes, b"a\nb");

        let decoded = decode_string(r#""x":ascii"#).unwrap();
        assert_eq!(decoded.encoding, Encoding::Ascii);
        assert_eq!(decoded.bytes, b"x");

        // Multi-line normal strings drop the terminator and following indent.
        let decoded = decode_string("\"第一行，\n    还是第一行\"").unwrap();
        assert_eq!(
            String::from_utf8(decoded.bytes.clone()).unwrap(),
            "第一行，还是第一行"
        );

        // Verbatim strings keep everything and double quotes collapse.
        let decoded = decode_string("@\"C:\\Users\\test\"").unwrap();
        assert_eq!(decoded.bytes, b"C:\\Users\\test");
        let decoded = decode_string("@\"He said \"\"Hello\"\"\"").unwrap();
        assert_eq!(decoded.bytes, b"He said \"Hello\"");
    }

    #[test]
    fn utf16_strings_use_two_byte_units() {
        let decoded = decode_string("\"ab\":utf16").unwrap();
        assert_eq!(decoded.unit_size, 2);
        assert_eq!(decoded.bytes, vec![b'a', 0, b'b', 0]);
        assert_eq!(decoded.element_count(), 2);
    }
}
