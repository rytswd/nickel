//! The lexer, transforming an input string to a stream of tokens.
//!
//! A custom lexer is implemented in order to support arbitrary interpolated expressions, which is
//! not possible using LALRPOP's generated lexer. To see why, consider the following string:
//!
//! ```
//! "hello, I have 1 + ${ {a = "40"}.a } + 1 bananas."
//! ```
//!
//! Once the `${` token is encountered, the lexer has to switch back to lexing expressions as
//! usual. But at the end of the interpolated expression, `+ 1 bananas.` needs to be parsed as a
//! string again, and not as program tokens. Since the interpolated expression is arbitrary, it can
//! contains nested `{` and `}` (as here, with records) and strings which themselves have
//! interpolated expression, and so on.
//!
//! This is typically not lexable using only regular expressions. To handle this, the lexer
//! maintains the following state:
//!  - mode: a current mode, which can be either `Normal`, `Str` or `DollarBrace` (the latter being
//!    a less important transitional mode, see the comments in [`str_literal`]()'s code).
//!  - mode stack: a stack to save and restore modes.
//!
//!  The two following operations are performed on the state:
//!  - push-update: save the current mode on the stack, and switch to a new one.
//!  - pop: restore the previous mode from the stack.
//!
//! When entering a string, the `Str` mode is pushed. When a `${` is encountered in a string,
//! starting an interpolated expression, the normal mode is pushed. At each starting `{` in normal
//! mode, the normal mode is also pushed. At each closing '}', the previous mode is popped.
//!
//! When parsing an interpolated expression, the closing `}` (if any) matching the starting `${`
//! will pop the `Str` mode from the stack. Then, the lexer knows that it should not try to lex the
//! next tokens as normal Nickel expressions, but rather as a string.
use std::fmt;
use std::str::CharIndices;

/// A token generated by the lexer.
#[derive(Clone, PartialEq, Debug)]
pub enum Token<'input> {
    /// An identifier.
    Identifier(&'input str),
    /// A binary operator.
    BinaryOp(&'input str),
    /// A base type (Num, Str, etc.).
    Type(&'input str),

    /// A string literal (which does not contain interpolated expressions).
    StrLiteral(String),
    /// A number.
    NumLiteral(f64),

    If,
    Then,
    Else,
    Forall,
    In,
    Let,
    Switch,

    True,
    False,

    Comma,
    Colon,
    Dollar,
    Equals,
    SemiCol,
    Dot,
    DotDollar,
    DollarBracket,
    DollarEquals,
    DollarBrace,
    DoubleQuote,
    MinusDollar,
    Fun,
    Import,
    Pipe,
    SimpleArrow,
    DoubleArrow,
    Hash,
    Backtick,
    Underscore,

    Tag,
    Assume,
    Promise,
    Deflt,
    Contract,
    ContractDeflt,
    Docstring,

    IsZero,
    IsNum,
    IsBool,
    IsStr,
    IsFun,
    IsList,
    Blame,
    ChangePol,
    Polarity,
    GoDom,
    GoCodom,
    Wrap,
    Embed,
    MapRec,
    Seq,
    DeepSeq,
    Head,
    Tail,
    Length,

    Unwrap,
    HasField,
    Map,
    ElemAt,
    Merge,

    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    LAngleBracket,
    RAngleBracket,
}

/// The lexer mode.
///
/// See the general module description for more details.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Normal,
    Str,
    DollarBrace(usize),
}

/// Lexing error.
#[derive(Clone, PartialEq, Debug)]
pub enum LexicalError {
    /// A closing brace '}' does not match an opening brace '{'.
    UnmatchedCloseBrace(usize),
    /// A character does not match the beginning of any token.
    UnexpectedChar(usize),
    /// An alphanumeric character directly follows a number literal.
    NumThenIdent(usize),
    /// Invalid escape sequence in a string literal.
    InvalidEscapeSequence(usize),
    /// Unexpected end of input.
    UnexpectedEOF(Vec<String>),
}

/// User for error reporting.
impl<'input> fmt::Display for Token<'input> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let repr = match self {
            Token::Identifier(s) | Token::BinaryOp(s) | Token::Type(s) => {
                return write!(f, "{}", s)
            }
            Token::StrLiteral(s) => return write!(f, "{}", s),
            Token::NumLiteral(n) => return write!(f, "{}", n),

            Token::If => "if",
            Token::Then => "then",
            Token::Else => "else",
            Token::Forall => "forall",
            Token::In => "in",
            Token::Let => "let",
            Token::Switch => "switch",

            Token::True => "true",
            Token::False => "false",

            Token::Comma => ",",
            Token::Colon => ":",
            Token::Dollar => "$",
            Token::Equals => "=",
            Token::SemiCol => ";",
            Token::Dot => ".",
            Token::DotDollar => ".$",
            Token::DollarBracket => "$[",
            Token::DollarBrace => "${",
            Token::MinusDollar => "-$",
            Token::DollarEquals => "$=",
            Token::Fun => "fun",
            Token::Import => "import",
            Token::Pipe => "|",
            Token::SimpleArrow => "->",
            Token::DoubleArrow => "=>",
            Token::Hash => "#",
            Token::Backtick => "`",
            Token::Underscore => "_",
            Token::DoubleQuote => "\"",

            Token::Tag => "tag",
            Token::Assume => "Assume(",
            Token::Promise => "Promise(",
            Token::Deflt => "Default(",
            Token::Contract => "Contract(",
            Token::ContractDeflt => "ContractDefault(",
            Token::Docstring => "Docstring(",

            Token::IsZero => "isZero",
            Token::IsNum => "isNum",
            Token::IsBool => "isBool",
            Token::IsStr => "isStr",
            Token::IsFun => "isFun",
            Token::IsList => "isList",
            Token::Blame => "blame",
            Token::ChangePol => "chngPol",
            Token::Polarity => "polarity",
            Token::GoDom => "goDom",
            Token::GoCodom => "goCodom",
            Token::Wrap => "wrap",
            Token::Unwrap => "unwrap",
            Token::Embed => "embed",
            Token::MapRec => "mapRec",
            Token::Seq => "seq",
            Token::DeepSeq => "deepSeq",
            Token::Head => "head",
            Token::Tail => "tail",
            Token::Length => "length",

            Token::HasField => "hasField",
            Token::Map => "map",
            Token::ElemAt => "elemAt",
            Token::Merge => "merge",

            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::LAngleBracket => "<",
            Token::RAngleBracket => ">",
        };
        write!(f, "{}", repr)
    }
}

/// The lexer state.
///
/// It maintains one character look-ahead, which is currently sufficient to decide tokens without
/// ambiguity.
pub struct Lexer<'input> {
    input: &'input str,
    chars: CharIndices<'input>,
    look_ahead: Option<(usize, char)>,
    mode_stack: Vec<Mode>,
    mode: Mode,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        let mut chars = input.char_indices();
        let look_ahead = chars.next();
        Lexer {
            input,
            chars,
            look_ahead,
            mode_stack: Vec::new(),
            mode: Mode::Normal,
        }
    }
}

pub type Spanned<'input> = (usize, Token<'input>, usize);

fn is_ident_start(chr: char, look_ahead: Option<char>) -> bool {
    match chr {
        'a'..='z' | 'A'..='Z' => true,
        '_' => look_ahead.map(is_ident_char).unwrap_or(false),
        _ => false,
    }
}

fn is_ident_char(chr: char) -> bool {
    match chr {
        '0'..='9' | '_' => true,
        chr => is_ident_start(chr, None),
    }
}

fn is_op_char(chr: char) -> bool {
    match chr {
        '+' | '@' | '=' | '-' | '<' | '>' | '.' | '|' | '#' => true,
        _ => false,
    }
}

fn is_whitespace(chr: char) -> bool {
    match chr {
        '\t' | '\n' | '\r' | ' ' => true,
        _ => false,
    }
}

// Digit can start with a heading '-'
fn is_num_start(chr: char, look_ahead: Option<char>) -> bool {
    match chr {
        '-' => look_ahead.map(is_digit).unwrap_or(false),
        chr => is_digit(chr),
    }
}

fn is_digit(chr: char) -> bool {
    match chr {
        '0'..='9' => true,
        _ => false,
    }
}

fn escape_char(chr: char) -> Option<char> {
    match chr {
        '\'' => Some('\''),
        '"' => Some('"'),
        '\\' => Some('\\'),
        '$' => Some('$'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        _ => None,
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Result<Spanned<'input>, LexicalError>;

    /// Return the next token of the input.
    fn next(&mut self) -> Option<Self::Item> {
        // This is a special case to avoid two characters look-ahead for dollar brace. See the
        // comments in str_literal()
        if let Mode::DollarBrace(index) = self.mode {
            assert!(self.pop_mode());
            self.push_mode(Mode::Normal);
            return Some(Ok((index, Token::DollarBrace, index + 2)));
        }

        // If we land here in Str mode, this means either:
        // 1. The previous token is a string literal, and the next to come is the closing double
        //    quote.
        // 2. Or We started lexing a string literal, encountered a '${', lexed the interpolated
        //    expression inside, and the previous token was the closing '}' which popped the Str
        //    mode back from the stack.
        // We peek one character to see if it is a closing double quote.
        // If not, we call str_literal before actually consuming any character.
        if self.mode == Mode::Str {
            return match self.look_ahead {
                Some((index, '"')) => {
                    self.consume();
                    assert!(self.pop_mode());
                    Some(Ok((index, Token::DoubleQuote, index + 1)))
                }
                Some((index, _)) => Some(self.str_literal(index)),
                None => None,
            };
        }

        while let Some((index, chr)) = self.consume() {
            let token = match chr {
                ',' => Ok((index, Token::Comma, index + 1)),
                ':' => Ok((index, Token::Colon, index + 1)),
                ';' => Ok((index, Token::SemiCol, index + 1)),
                '$' => match self.look_ahead {
                    Some((_, '=')) => {
                        self.consume();
                        Ok((index, Token::DollarEquals, index + 2))
                    }
                    Some((_, '[')) => {
                        self.consume();
                        Ok((index, Token::DollarBracket, index + 2))
                    }
                    Some((_, '{')) => {
                        self.consume();
                        Ok((index, Token::DollarBrace, index + 2))
                    }
                    _ => Ok((index, Token::Dollar, index + 1)),
                },
                '{' => {
                    self.push_mode(Mode::Normal);
                    Ok((index, Token::LBrace, index + 1))
                }
                '}' => {
                    if !self.pop_mode() {
                        Err(LexicalError::UnmatchedCloseBrace(index))
                    } else {
                        Ok((index, Token::RBrace, index + 1))
                    }
                }
                '[' => Ok((index, Token::LBracket, index + 1)),
                ']' => Ok((index, Token::RBracket, index + 1)),
                '(' => Ok((index, Token::LParen, index + 1)),
                ')' => Ok((index, Token::RParen, index + 1)),
                '#' => Ok((index, Token::Hash, index + 1)),
                '`' => Ok((index, Token::Backtick, index + 1)),
                '"' => {
                    self.push_mode(Mode::Str);
                    Ok((index, Token::DoubleQuote, index + 1))
                }
                chr if is_ident_start(chr, self.look_ahead.map(|(_, chr)| chr)) => {
                    self.identifier(index)
                }
                '_' => Ok((index, Token::Underscore, index + 1)),
                chr if is_num_start(chr, self.look_ahead.map(|(_, chr)| chr)) => {
                    self.num_literal(index)
                }
                chr if is_op_char(chr) => self.operator(index),
                // Ignore whitespaces
                chr if is_whitespace(chr) => continue,
                _ => Err(LexicalError::UnexpectedChar(index)),
            };

            return Some(token);
        }

        None
    }
}

impl<'input> Lexer<'input> {
    /// Save the current mode on the stack, and then set it the `mode`.
    fn push_mode(&mut self, mode: Mode) {
        self.mode_stack.push(self.mode);
        self.mode = mode;
    }

    /// Restore the previous mode from the stack. Return false if the stack was empty.
    fn pop_mode(&mut self) -> bool {
        if let Some(mode) = self.mode_stack.pop() {
            self.mode = mode;
            true
        } else {
            self.mode = Mode::Normal;
            false
        }
    }

    /// Take the next character from the stream.
    fn consume(&mut self) -> Option<(usize, char)> {
        std::mem::replace(&mut self.look_ahead, self.chars.next())
    }

    /// Check if the next character is equal to the given parameter without consuming.
    fn look_ahead_is(&self, chr: char) -> bool {
        match self.look_ahead {
            Some((_, next)) if next == chr => true,
            _ => false,
        }
    }

    /// Consume the coming characters while they satisfy some predicate. Return the
    /// position of the first character which does not satisfy `pred`, together with a slice of the
    /// input starting at `start` until this character (excluded).
    ///
    /// # Example
    /// In the following situation:
    ///
    /// ```
    /// 1234.456 ab
    /// ^    ^  ^
    /// s    c  end
    /// ```
    ///
    /// where `c` is the current position in the stream, then
    /// `self.take_while(s, is_digit)` returns `(end, "1234.456")` and set the current position of
    /// the stream to `end`.
    fn take_while<F>(&mut self, start: usize, pred: F) -> (usize, &'input str)
    where
        F: Fn(char) -> bool,
    {
        let mut end = start;

        while let Some((index, chr)) = self.look_ahead {
            end = index;

            if pred(chr) {
                self.consume();
            } else {
                return (index, &self.input[start..index]);
            }
        }

        end += 1;
        (end, &self.input[start..end])
    }

    /// Try to lex the next token as an identifier.
    ///
    /// If the identifier matches a reserved keyword, the associated token is returned instead.
    pub fn identifier(&mut self, start: usize) -> Result<Spanned<'input>, LexicalError> {
        let (mut end, slice) = self.take_while(start, is_ident_char);

        // Reserved keywords are just special identifiers.
        let is_next_lparen = self.look_ahead_is('(');
        let token = match slice {
            "if" => Token::If,
            "then" => Token::Then,
            "else" => Token::Else,
            "forall" => Token::Forall,
            "in" => Token::In,
            "let" => Token::Let,
            "switch" => Token::Switch,
            "tag" => Token::Tag,
            "fun" => Token::Fun,
            "import" => Token::Import,
            "true" => Token::True,
            "false" => Token::False,
            "Assume" if is_next_lparen => {
                self.consume();
                end += 1;
                Token::Assume
            }
            "Promise" if is_next_lparen => {
                self.consume();
                end += 1;
                Token::Promise
            }
            "Default" if is_next_lparen => {
                self.consume();
                end += 1;
                Token::Deflt
            }
            "Contract" if is_next_lparen => {
                self.consume();
                end += 1;
                Token::Contract
            }
            "ContractDefault" if is_next_lparen => {
                self.consume();
                end += 1;
                Token::ContractDeflt
            }
            "Docstring" if is_next_lparen => {
                self.consume();
                end += 1;
                Token::Docstring
            }
            "isZero" => Token::IsZero,
            "isNum" => Token::IsNum,
            "isBool" => Token::IsBool,
            "isStr" => Token::IsStr,
            "isFun" => Token::IsFun,
            "isList" => Token::IsList,
            "blame" => Token::Blame,
            "chngPol" => Token::ChangePol,
            "polarity" => Token::Polarity,
            "goDom" => Token::GoDom,
            "goCodom" => Token::GoCodom,
            "wrap" => Token::Wrap,
            "embed" => Token::Embed,
            "mapRec" => Token::MapRec,
            "seq" => Token::Seq,
            "deepSeq" => Token::DeepSeq,
            "head" => Token::Head,
            "tail" => Token::Tail,
            "length" => Token::Length,
            "unwrap" => Token::Unwrap,
            "hasField" => Token::HasField,
            "map" => Token::Map,
            "elemAt" => Token::ElemAt,
            "merge" => Token::Merge,
            ty @ "Dyn" | ty @ "Num" | ty @ "Bool" | ty @ "Str" | ty @ "List" => Token::Type(ty),
            id => Token::Identifier(id),
        };

        Ok((start, token, end))
    }

    /// Try to lex the next token as a number literal.
    pub fn num_literal(&mut self, start: usize) -> Result<Spanned<'input>, LexicalError> {
        let (end, num) = self.take_while(start, is_digit);

        // Take the fractional part into account, if there is one
        let (end, num) = match self.look_ahead {
            Some((_, '.')) => {
                self.consume();
                self.take_while(start, is_digit)
            }
            _ => (end, num),
        };

        match self.look_ahead {
            // Number literals must not be followed directly by an identifier character
            Some((index, chr)) if is_ident_char(chr) => Err(LexicalError::NumThenIdent(index)),
            _ => Ok((start, Token::NumLiteral(num.parse().unwrap()), end)),
        }
    }

    /// Try the lex the next token as an operator.
    ///
    /// As for identifiers, if the operator correspond to a reserved symbol of the language, the
    /// associated token is returned instead.
    pub fn operator(&mut self, start: usize) -> Result<Spanned<'input>, LexicalError> {
        let (mut end, op) = self.take_while(start, is_op_char);

        let token = match op {
            "." if self.look_ahead_is('$') => {
                self.consume();
                end += 1;
                Token::DotDollar
            }
            "." => Token::Dot,
            "-" if self.look_ahead_is('$') => {
                self.consume();
                end += 1;
                Token::MinusDollar
            }
            "=" => Token::Equals,
            "->" => Token::SimpleArrow,
            "=>" => Token::DoubleArrow,
            "<" => Token::LAngleBracket,
            ">" => Token::RAngleBracket,
            "|" => Token::Pipe,
            op => Token::BinaryOp(op),
        };

        Ok((start, token, end))
    }

    /// Try to lex the next token as a string literal.
    pub fn str_literal(&mut self, start: usize) -> Result<Spanned<'input>, LexicalError> {
        let mut eof = start + 1;
        let mut acc = String::new();

        loop {
            if self.look_ahead_is('"') {
                return Ok((start, Token::StrLiteral(acc), start + 1));
            }

            if let Some((index, chr)) = self.consume() {
                eof = index + 1;
                match chr {
                    '\\' => {
                        let (i, c) = self.consume().ok_or(LexicalError::UnexpectedEOF(vec![
                            String::from("escape sequence"),
                        ]))?;
                        acc.push(
                            escape_char(c).ok_or_else(|| LexicalError::InvalidEscapeSequence(i))?,
                        );
                    }
                    '$' => {
                        if self.look_ahead_is('{') {
                            self.consume();

                            // Instead of returning an empty string token, directly return the
                            // dollar brace.
                            if acc.is_empty() {
                                self.push_mode(Mode::Normal);
                                return Ok((index, Token::DollarBrace, index + 2));
                            } else {
                                // This is the only point where we would actually need to look two
                                // characters ahead, to determine if the coming token is a '${'. We
                                // can not, and had to consume the '$' of '${' to decide. To avoid
                                // using a 2 chars look-ahead buffer just for this, we encode this
                                // special case in Mode. Mode::DollarBrace indicates precisely that
                                // we were lexing a string literal, and that we encountered and
                                // consumed a "${", that should be returned without consuming
                                // anything at the next call to next()
                                self.push_mode(Mode::DollarBrace(index));
                                return Ok((start, Token::StrLiteral(acc), index));
                            }
                        } else {
                            acc.push('$');
                        }
                    }
                    chr => acc.push(chr),
                }
            } else {
                // We could fail here as we reached EOF while lexing a string, meaning the string
                // is not terminated. However, we prefer to let the parser handle the problem
                // instead of adding special cases in the lexer, as this is not the only code path
                // which implies an unterminated string.
                return Ok((start, Token::StrLiteral(acc), eof));
            }
        }
    }
}
