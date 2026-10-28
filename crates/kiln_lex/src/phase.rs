//! Translation phases 1 and 2 ([lex.phases]/1.1-1.2) as an on-the-fly
//! decoder: the lexer never materializes the spliced text, it asks for the
//! next *decoded* character at a byte position and gets back how many
//! source bytes that character occupied (more than its UTF-8 length when
//! line splices or a trigraph were involved).

/// A source text viewed through phases 1-2.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Src<'a> {
    pub(crate) text: &'a str,
    pub(crate) bytes: &'a [u8],
    pub(crate) trigraphs: bool,
}

/// Horizontal whitespace allowed between `\` and the newline of a splice
/// (clang's extension, diagnosed as `BackslashNewlineSpace`).
#[inline]
pub(crate) fn is_hws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | 0x0B | 0x0C)
}

/// The character a trigraph `??x` stands for ([lex.trigraph], C++14).
pub(crate) fn trigraph_char(x: u8) -> Option<char> {
    Some(match x {
        b'=' => '#',
        b'/' => '\\',
        b'\'' => '^',
        b'(' => '[',
        b')' => ']',
        b'!' => '|',
        b'<' => '{',
        b'>' => '}',
        b'-' => '~',
        _ => return None,
    })
}

impl<'a> Src<'a> {
    pub(crate) fn new(text: &'a str, trigraphs: bool) -> Self {
        Src { text, bytes: text.as_bytes(), trigraphs }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub(crate) fn byte(&self, p: usize) -> Option<u8> {
        self.bytes.get(p).copied()
    }

    /// The trigraph spelled at `p`, regardless of whether trigraphs are
    /// enabled.
    pub(crate) fn trigraph_at(&self, p: usize) -> Option<char> {
        if self.byte(p) == Some(b'?') && self.byte(p + 1) == Some(b'?') {
            trigraph_char(self.byte(p + 2)?)
        } else {
            None
        }
    }

    /// If a line splice starts at `p` (`\`, or `??/` with trigraphs, then
    /// optional horizontal whitespace, then a newline), returns the
    /// position after it and whether whitespace preceded the newline.
    pub(crate) fn splice_at(&self, p: usize) -> Option<(usize, bool)> {
        let after = match self.byte(p)? {
            b'\\' => p + 1,
            b'?' if self.trigraphs && self.trigraph_at(p) == Some('\\') => p + 3,
            _ => return None,
        };
        let mut q = after;
        while self.byte(q).is_some_and(is_hws) {
            q += 1;
        }
        match self.byte(q)? {
            b'\n' => Some((q + 1, q != after)),
            b'\r' => {
                Some((if self.byte(q + 1) == Some(b'\n') { q + 2 } else { q + 1 }, q != after))
            }
            _ => None,
        }
    }

    /// Skips any number of consecutive line splices starting at `p`.
    pub(crate) fn skip_splices(&self, mut p: usize) -> usize {
        while let Some((q, _)) = self.splice_at(p) {
            p = q;
        }
        p
    }

    /// The decoded character at `p` and the number of source bytes it
    /// occupies (including line splices before it and the trigraph that
    /// spells it). `None` at end of input; the size then counts trailing
    /// splices.
    #[inline]
    pub(crate) fn decode(&self, p: usize) -> (Option<char>, usize) {
        match self.byte(p) {
            None => (None, 0),
            Some(b) if b < 0x80 && b != b'\\' && b != b'?' => (Some(char::from(b)), 1),
            Some(_) => self.decode_slow(p),
        }
    }

    #[cold]
    fn decode_slow(&self, p: usize) -> (Option<char>, usize) {
        let q = self.skip_splices(p);
        let Some(b) = self.byte(q) else {
            return (None, q - p);
        };
        if b == b'?'
            && self.trigraphs
            && let Some(c) = self.trigraph_at(q)
        {
            return (Some(c), q + 3 - p);
        }
        if b < 0x80 {
            return (Some(char::from(b)), q + 1 - p);
        }
        match self.text.get(q..).and_then(|s| s.chars().next()) {
            Some(c) => (Some(c), q + c.len_utf8() - p),
            // Not on a character boundary: cannot happen for positions the
            // lexer produces, but never panic on it.
            None => (Some(char::REPLACEMENT_CHARACTER), q + 1 - p),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Src;

    #[test]
    fn decode_splices_and_trigraphs() {
        let s = Src::new("a\\\nb\\ \r\nc??/\nd??=", true);
        assert_eq!(s.decode(0), (Some('a'), 1));
        assert_eq!(s.decode(1), (Some('b'), 3));
        assert_eq!(s.decode(4), (Some('c'), 5));
        assert_eq!(s.splice_at(4), Some((8, true)));
        assert_eq!(s.decode(9), (Some('d'), 5));
        assert_eq!(s.decode(14), (Some('#'), 3));
        assert_eq!(s.decode(17), (None, 0));
        let off = Src::new("??=", false);
        assert_eq!(off.decode(0), (Some('?'), 1));
        assert_eq!(Src::new("x\\\n", false).decode(1), (None, 2));
        assert_eq!(Src::new("é", false).decode(0), (Some('é'), 2));
    }
}
