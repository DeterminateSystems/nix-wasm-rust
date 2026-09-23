//! Minimal, three-valued evaluation of preprocessor conditionals.
//!
//! Conditions are evaluated against a set of macros known to be defined
//! (with a value) or known to be undefined. Anything else is unknown, in
//! which case both branches of a conditional are taken, so that the set of
//! includes found is an over-approximation of what the compiler will see.

use std::collections::{HashMap, HashSet};

pub struct Defines {
    /// Macros known to be defined, with their value.
    pub defined: HashMap<String, String>,
    /// Macros known to be undefined.
    pub undefined: HashSet<String>,
}

/// A value in a `#if` expression: a known integer, or unknown.
type Val = Option<i64>;

fn truth(v: Val) -> Option<bool> {
    v.map(|n| n != 0)
}

fn from_bool(b: bool) -> Val {
    Some(b as i64)
}

fn and(a: Val, b: Val) -> Val {
    match (truth(a), truth(b)) {
        (Some(false), _) | (_, Some(false)) => Some(0),
        (Some(true), Some(true)) => Some(1),
        _ => None,
    }
}

fn or(a: Val, b: Val) -> Val {
    match (truth(a), truth(b)) {
        (Some(true), _) | (_, Some(true)) => Some(1),
        (Some(false), Some(false)) => Some(0),
        _ => None,
    }
}

fn not(a: Val) -> Val {
    truth(a).map(|b| !b as i64)
}

/// Whether a conditional is taken: `Some(true)`, `Some(false)`, or
/// `None` if it cannot be decided.
pub fn eval_condition(expr: &str, defines: &Defines) -> Option<bool> {
    let tokens = tokenize(expr);
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        defines,
    };
    let val = parser.parse_or();
    if parser.pos != tokens.len() {
        // Trailing garbage: don't pretend to understand the expression.
        return None;
    }
    truth(val)
}

#[derive(Debug, PartialEq)]
enum Token {
    Ident(String),
    Num(i64),
    Op(&'static str),
    Unknown,
}

const OPS: &[&str] = &[
    "&&", "||", "==", "!=", "<=", ">=", "<", ">", "!", "(", ")", "+", "-", "*", "/", "%",
];

fn tokenize(s: &str) -> Vec<Token> {
    let mut tokens = vec![];
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            tokens.push(Token::Ident(s[start..i].to_string()));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                i += 1;
            }
            tokens.push(parse_number(&s[start..i]));
        } else if let Some(op) = OPS.iter().find(|op| s[i..].starts_with(*op)) {
            tokens.push(Token::Op(op));
            i += op.len();
        } else {
            tokens.push(Token::Unknown);
            i += 1;
        }
    }
    tokens
}

fn parse_number(s: &str) -> Token {
    // Strip integer suffixes like `L`, `UL`, `u`.
    let digits = s.trim_end_matches(|c: char| matches!(c, 'u' | 'U' | 'l' | 'L'));
    let parsed = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).ok()
    } else if digits.len() > 1 && digits.starts_with('0') {
        i64::from_str_radix(&digits[1..], 8).ok()
    } else {
        digits.parse().ok()
    };
    match parsed {
        Some(n) => Token::Num(n),
        None => Token::Unknown,
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    defines: &'a Defines,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.peek()
            == Some(&Token::Op(
                OPS.iter().find(|o| **o == op).copied().unwrap_or(""),
            ))
        {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> Val {
        let mut lhs = self.parse_and();
        while self.eat_op("||") {
            let rhs = self.parse_and();
            lhs = or(lhs, rhs);
        }
        lhs
    }

    fn parse_and(&mut self) -> Val {
        let mut lhs = self.parse_equality();
        while self.eat_op("&&") {
            let rhs = self.parse_equality();
            lhs = and(lhs, rhs);
        }
        lhs
    }

    fn parse_equality(&mut self) -> Val {
        let mut lhs = self.parse_relational();
        loop {
            if self.eat_op("==") {
                let rhs = self.parse_relational();
                lhs = lhs.zip(rhs).map(|(a, b)| (a == b) as i64);
            } else if self.eat_op("!=") {
                let rhs = self.parse_relational();
                lhs = lhs.zip(rhs).map(|(a, b)| (a != b) as i64);
            } else {
                return lhs;
            }
        }
    }

    fn parse_relational(&mut self) -> Val {
        let mut lhs = self.parse_additive();
        loop {
            let op = match self.peek() {
                Some(Token::Op(op @ ("<" | ">" | "<=" | ">="))) => *op,
                _ => return lhs,
            };
            self.pos += 1;
            let rhs = self.parse_additive();
            lhs = lhs.zip(rhs).map(|(a, b)| {
                (match op {
                    "<" => a < b,
                    ">" => a > b,
                    "<=" => a <= b,
                    _ => a >= b,
                }) as i64
            });
        }
    }

    fn parse_additive(&mut self) -> Val {
        let mut lhs = self.parse_multiplicative();
        loop {
            if self.eat_op("+") {
                let rhs = self.parse_multiplicative();
                lhs = lhs.zip(rhs).and_then(|(a, b)| a.checked_add(b));
            } else if self.eat_op("-") {
                let rhs = self.parse_multiplicative();
                lhs = lhs.zip(rhs).and_then(|(a, b)| a.checked_sub(b));
            } else {
                return lhs;
            }
        }
    }

    fn parse_multiplicative(&mut self) -> Val {
        let mut lhs = self.parse_unary();
        loop {
            if self.eat_op("*") {
                let rhs = self.parse_unary();
                lhs = lhs.zip(rhs).and_then(|(a, b)| a.checked_mul(b));
            } else if self.eat_op("/") {
                let rhs = self.parse_unary();
                lhs = lhs.zip(rhs).and_then(|(a, b)| a.checked_div(b));
            } else if self.eat_op("%") {
                let rhs = self.parse_unary();
                lhs = lhs.zip(rhs).and_then(|(a, b)| a.checked_rem(b));
            } else {
                return lhs;
            }
        }
    }

    fn parse_unary(&mut self) -> Val {
        if self.eat_op("!") {
            not(self.parse_unary())
        } else if self.eat_op("-") {
            self.parse_unary().and_then(|a| a.checked_neg())
        } else if self.eat_op("+") {
            self.parse_unary()
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Val {
        match self.peek() {
            Some(Token::Num(n)) => {
                self.pos += 1;
                Some(*n)
            }
            Some(Token::Op("(")) => {
                self.pos += 1;
                let val = self.parse_or();
                if self.eat_op(")") {
                    val
                } else {
                    None
                }
            }
            Some(Token::Ident(name)) if name == "defined" => {
                self.pos += 1;
                let parenthesized = self.eat_op("(");
                let val = match self.peek() {
                    Some(Token::Ident(name)) => {
                        self.pos += 1;
                        self.is_defined(name)
                    }
                    _ => None,
                };
                if parenthesized && !self.eat_op(")") {
                    return None;
                }
                val
            }
            Some(Token::Ident(name)) => {
                self.pos += 1;
                match name.as_str() {
                    "true" => Some(1),
                    "false" => Some(0),
                    _ => self.value_of(name),
                }
            }
            _ => {
                // Consume the token so that the parser makes progress; the
                // result is unknown anyway.
                self.pos += 1;
                None
            }
        }
    }

    fn is_defined(&self, name: &str) -> Val {
        if self.defines.defined.contains_key(name) {
            from_bool(true)
        } else if self.defines.undefined.contains(name) {
            from_bool(false)
        } else {
            None
        }
    }

    /// The value of a macro in an expression: undefined macros are 0, and
    /// defined macros must have an integer value to be known.
    fn value_of(&self, name: &str) -> Val {
        if let Some(value) = self.defines.defined.get(name) {
            match tokenize(value).as_slice() {
                [Token::Num(n)] => Some(*n),
                [Token::Ident(other)] if other != name => self.value_of(other),
                _ => None,
            }
        } else if self.defines.undefined.contains(name) {
            Some(0)
        } else {
            None
        }
    }
}

/// Tracks nested `#if` blocks while scanning a file.
pub struct ConditionalStack {
    frames: Vec<Frame>,
}

struct Frame {
    /// Whether the enclosing context is active.
    parent_active: bool,
    /// Whether an earlier branch of this `#if` chain was definitely taken
    /// (`Some(true)`), definitely not (`Some(false)`), or unknown.
    any_taken: Option<bool>,
    /// Whether the current branch is (possibly) active.
    active: bool,
}

impl ConditionalStack {
    pub fn new() -> Self {
        ConditionalStack { frames: vec![] }
    }

    /// Whether directives at this point may be seen by the compiler.
    pub fn active(&self) -> bool {
        self.frames.last().map_or(true, |f| f.active)
    }

    pub fn push(&mut self, cond: Option<bool>) {
        let parent_active = self.active();
        self.frames.push(Frame {
            parent_active,
            any_taken: cond,
            active: parent_active && cond != Some(false),
        });
    }

    pub fn elif(&mut self, cond: Option<bool>) {
        if let Some(frame) = self.frames.last_mut() {
            if frame.any_taken == Some(true) {
                frame.active = false;
            } else {
                frame.active = frame.parent_active && cond != Some(false);
                frame.any_taken = match (frame.any_taken, cond) {
                    (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                };
            }
        }
    }

    pub fn else_(&mut self) {
        if let Some(frame) = self.frames.last_mut() {
            frame.active = frame.parent_active && frame.any_taken != Some(true);
        }
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defines() -> Defines {
        Defines {
            defined: [
                ("__linux__", "1"),
                ("HAVE_SECCOMP", "1"),
                ("HAVE_AWS", "0"),
                ("VERSION", "0x10"),
                ("ALIAS", "VERSION"),
                ("NAME", "\"nix\""),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            undefined: ["_WIN32", "__APPLE__"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    fn eval(expr: &str) -> Option<bool> {
        eval_condition(expr, &defines())
    }

    #[test]
    fn defined_operator() {
        assert_eq!(eval("defined(__linux__)"), Some(true));
        assert_eq!(eval("defined __linux__"), Some(true));
        assert_eq!(eval("defined(_WIN32)"), Some(false));
        assert_eq!(eval("!defined(_WIN32)"), Some(true));
        assert_eq!(eval("defined(__GNUC__)"), None);
        assert_eq!(eval("!defined(__GNUC__)"), None);
    }

    #[test]
    fn macro_values() {
        assert_eq!(eval("HAVE_SECCOMP"), Some(true));
        assert_eq!(eval("HAVE_AWS"), Some(false));
        // Undefined macros evaluate to 0.
        assert_eq!(eval("_WIN32"), Some(false));
        // Unknown macros are unknown.
        assert_eq!(eval("__GNUC__"), None);
        // Macros expanding to other macros, and non-integer values.
        assert_eq!(eval("ALIAS == 16"), Some(true));
        assert_eq!(eval("NAME"), None);
    }

    #[test]
    fn logic_with_unknowns() {
        // Unknown operands only matter if the known one doesn't decide.
        assert_eq!(eval("defined(_WIN32) && defined(__GNUC__)"), Some(false));
        assert_eq!(eval("defined(__GNUC__) && defined(_WIN32)"), Some(false));
        assert_eq!(eval("defined(__linux__) || defined(__GNUC__)"), Some(true));
        assert_eq!(eval("defined(__GNUC__) || defined(__linux__)"), Some(true));
        assert_eq!(eval("defined(__linux__) && defined(__GNUC__)"), None);
        assert_eq!(eval("defined(_WIN32) || defined(__GNUC__)"), None);
        assert_eq!(
            eval("defined(__linux__) || defined(__FreeBSD__)"),
            Some(true)
        );
        assert_eq!(eval("defined(__APPLE__) || defined(__FreeBSD__)"), None);
    }

    #[test]
    fn comparisons_and_arithmetic() {
        assert_eq!(eval("VERSION >= 16"), Some(true));
        assert_eq!(eval("VERSION < 16"), Some(false));
        assert_eq!(eval("VERSION != 0x10"), Some(false));
        assert_eq!(eval("VERSION * 2 + 1 == 33"), Some(true));
        assert_eq!(eval("(VERSION - 16) % 3 == 0"), Some(true));
        assert_eq!(eval("-VERSION < 0"), Some(true));
        assert_eq!(eval("__GNUC__ >= 12"), None);
        assert_eq!(eval("__GNUC__ * 0 == 0"), None);
        assert_eq!(eval("1 / 0"), None);
    }

    #[test]
    fn literals() {
        assert_eq!(eval("0"), Some(false));
        assert_eq!(eval("1"), Some(true));
        assert_eq!(eval("010 == 8"), Some(true));
        assert_eq!(eval("0x1fUL == 31"), Some(true));
        assert_eq!(eval("true"), Some(true));
        assert_eq!(eval("false"), Some(false));
    }

    #[test]
    fn unparsable_is_unknown() {
        assert_eq!(eval(""), None);
        assert_eq!(eval("defined("), None);
        assert_eq!(eval("(1"), None);
        assert_eq!(eval("1 1"), None);
        assert_eq!(eval("__has_include(<foo.h>)"), None);
        assert_eq!(eval("SIZEOF(int) == 4"), None);
        // Syntax the parser does not understand makes the whole expression
        // unknown, even if a known operand would have decided it.
        assert_eq!(eval("0 && SIZEOF(int) == 4"), None);
        // ...unlike an unknown macro, which is a valid operand.
        assert_eq!(eval("0 && UNKNOWN == 4"), Some(false));
    }

    #[test]
    fn stack_if_else() {
        let mut s = ConditionalStack::new();
        assert!(s.active());
        s.push(Some(false));
        assert!(!s.active());
        s.else_();
        assert!(s.active());
        s.pop();
        assert!(s.active());

        s.push(Some(true));
        assert!(s.active());
        s.elif(Some(true));
        assert!(!s.active());
        s.else_();
        assert!(!s.active());
        s.pop();
    }

    #[test]
    fn stack_unknown_keeps_both_branches() {
        let mut s = ConditionalStack::new();
        s.push(None);
        assert!(s.active());
        s.elif(Some(false));
        assert!(!s.active());
        s.elif(None);
        assert!(s.active());
        s.else_();
        assert!(s.active());
        s.pop();

        // A definitely taken branch after an unknown one closes the chain.
        s.push(None);
        s.elif(Some(true));
        assert!(s.active());
        s.else_();
        assert!(!s.active());
        s.pop();
    }

    #[test]
    fn stack_nesting() {
        let mut s = ConditionalStack::new();
        s.push(Some(false));
        s.push(Some(true));
        assert!(!s.active(), "inner #if inside a dead branch is dead");
        s.else_();
        assert!(!s.active());
        s.pop();
        s.else_();
        assert!(s.active());
        s.push(None);
        assert!(s.active());
        s.pop();
        s.pop();
        assert!(s.active());
    }
}
