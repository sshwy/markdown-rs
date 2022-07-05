//! Several helpers to parse whitespace (`space_or_tab`, `space_or_tab_eol`).
//!
//! ## References
//!
//! *   [`micromark-factory-space/index.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-factory-space/dev/index.js)

use crate::subtokenize::link;
use crate::tokenizer::{Code, ContentType, State, StateFn, StateFnResult, TokenType, Tokenizer};

/// Options to parse `space_or_tab`.
#[derive(Debug)]
pub struct Options {
    /// Minimum allowed characters (inclusive).
    pub min: usize,
    /// Maximum allowed characters (inclusive).
    pub max: usize,
    /// Token type to use for whitespace events.
    pub kind: TokenType,
    /// Connect this whitespace to the previous.
    pub connect: bool,
    /// Embedded content type to use.
    pub content_type: Option<ContentType>,
}

/// Options to parse `space_or_tab` and one optional eol, but no blank line.
#[derive(Debug)]
pub struct EolOptions {
    /// Connect this whitespace to the previous.
    pub connect: bool,
    /// Embedded content type to use.
    pub content_type: Option<ContentType>,
}

/// State needed to parse `space_or_tab`.
#[derive(Debug)]
struct Info {
    /// Current size.
    size: usize,
    /// Configuration.
    options: Options,
}

/// State needed to parse `space_or_tab_eol`.
#[derive(Debug)]
struct EolInfo {
    /// Whether to connect the next whitespace to the event before.
    connect: bool,
    /// Whether there was initial whitespace.
    ok: bool,
    /// Configuration.
    options: EolOptions,
}

/// One or more `space_or_tab`.
///
/// ```bnf
/// space_or_tab ::= 1*( ' ' '\t' )
/// ```
pub fn space_or_tab() -> Box<StateFn> {
    space_or_tab_min_max(1, usize::MAX)
}

/// Between `x` and `y` `space_or_tab`.
///
/// ```bnf
/// space_or_tab_min_max ::= x*y( ' ' '\t' )
/// ```
pub fn space_or_tab_min_max(min: usize, max: usize) -> Box<StateFn> {
    space_or_tab_with_options(Options {
        kind: TokenType::SpaceOrTab,
        min,
        max,
        content_type: None,
        connect: false,
    })
}

/// `space_or_tab`, with the given options.
pub fn space_or_tab_with_options(options: Options) -> Box<StateFn> {
    Box::new(|t, c| start(t, c, Info { size: 0, options }))
}

/// `space_or_tab`, or optionally `space_or_tab`, one `eol`, and
/// optionally `space_or_tab`.
///
/// ```bnf
/// space_or_tab_eol ::= 1*( ' ' '\t' ) | 0*( ' ' '\t' ) eol 0*( ' ' '\t' )
/// ```
pub fn space_or_tab_eol() -> Box<StateFn> {
    space_or_tab_eol_with_options(EolOptions {
        content_type: None,
        connect: false,
    })
}

/// `space_or_tab_eol`, with the given options.
pub fn space_or_tab_eol_with_options(options: EolOptions) -> Box<StateFn> {
    Box::new(move |tokenizer, code| {
        let mut info = EolInfo {
            connect: false,
            ok: false,
            options,
        };

        tokenizer.attempt(
            space_or_tab_with_options(Options {
                kind: TokenType::SpaceOrTab,
                min: 1,
                max: usize::MAX,
                content_type: info.options.content_type,
                connect: info.options.connect,
            }),
            move |ok| {
                if ok {
                    info.ok = ok;

                    if info.options.content_type.is_some() {
                        info.connect = true;
                    }
                }

                Box::new(|t, c| after_space_or_tab(t, c, info))
            },
        )(tokenizer, code)
    })
}

/// Before `space_or_tab`.
///
/// ```markdown
/// alpha| bravo
/// ```
fn start(tokenizer: &mut Tokenizer, code: Code, mut info: Info) -> StateFnResult {
    match code {
        Code::VirtualSpace | Code::Char('\t' | ' ') if info.options.max > 0 => {
            tokenizer.enter_with_content(info.options.kind.clone(), info.options.content_type);

            if info.options.content_type.is_some() {
                let index = tokenizer.events.len() - 1;
                link(&mut tokenizer.events, index);
            }

            tokenizer.consume(code);
            info.size += 1;
            (State::Fn(Box::new(|t, c| inside(t, c, info))), None)
        }
        _ => (
            if info.options.min == 0 {
                State::Ok
            } else {
                State::Nok
            },
            Some(vec![code]),
        ),
    }
}

/// In `space_or_tab`.
///
/// ```markdown
/// alpha |bravo
/// alpha | bravo
/// ```
fn inside(tokenizer: &mut Tokenizer, code: Code, mut info: Info) -> StateFnResult {
    match code {
        Code::VirtualSpace | Code::Char('\t' | ' ') if info.size < info.options.max => {
            tokenizer.consume(code);
            info.size += 1;
            (State::Fn(Box::new(|t, c| inside(t, c, info))), None)
        }
        _ => {
            tokenizer.exit(info.options.kind.clone());
            (
                if info.size >= info.options.min {
                    State::Ok
                } else {
                    State::Nok
                },
                Some(vec![code]),
            )
        }
    }
}

/// `space_or_tab_eol`: after optionally first `space_or_tab`.
///
/// ```markdown
/// alpha |
/// bravo
/// ```
///
/// ```markdown
/// alpha|
/// bravo
/// ```
fn after_space_or_tab(tokenizer: &mut Tokenizer, code: Code, mut info: EolInfo) -> StateFnResult {
    match code {
        Code::CarriageReturnLineFeed | Code::Char('\n' | '\r') => {
            tokenizer.enter_with_content(TokenType::LineEnding, info.options.content_type);

            if info.connect {
                let index = tokenizer.events.len() - 1;
                link(&mut tokenizer.events, index);
            } else if info.options.content_type.is_some() {
                info.connect = true;
            }

            tokenizer.consume(code);
            tokenizer.exit(TokenType::LineEnding);
            (State::Fn(Box::new(|t, c| after_eol(t, c, info))), None)
        }
        _ if info.ok => (State::Ok, Some(vec![code])),
        _ => (State::Nok, None),
    }
}

/// `space_or_tab_eol`: after eol.
///
/// ```markdown
/// alpha
/// |bravo
/// ```
///
/// ```markdown
/// alpha
/// |bravo
/// ```
#[allow(clippy::needless_pass_by_value)]
fn after_eol(tokenizer: &mut Tokenizer, code: Code, info: EolInfo) -> StateFnResult {
    tokenizer.attempt_opt(
        space_or_tab_with_options(Options {
            kind: TokenType::SpaceOrTab,
            min: 1,
            max: usize::MAX,
            content_type: info.options.content_type,
            connect: info.connect,
        }),
        after_more_space_or_tab,
    )(tokenizer, code)
}

/// `space_or_tab_eol`: after more (optional) `space_or_tab`.
///
/// ```markdown
/// alpha
/// |bravo
/// ```
///
/// ```markdown
/// alpha
///  |bravo
/// ```
fn after_more_space_or_tab(_tokenizer: &mut Tokenizer, code: Code) -> StateFnResult {
    // Blank line not allowed.
    if matches!(
        code,
        Code::None | Code::CarriageReturnLineFeed | Code::Char('\n' | '\r')
    ) {
        (State::Nok, None)
    } else {
        (State::Ok, Some(vec![code]))
    }
}
