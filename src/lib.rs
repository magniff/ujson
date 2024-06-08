#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct State {
    current: usize,
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
enum ParserError {
    #[error("Parse error at position {0}")]
    NoParse(usize),
}

struct Parser<'input, R> {
    inner: Box<dyn FnOnce(&'input str, State) -> Result<(R, State), ParserError> + 'input>,
}

fn pattern<'input, 'pattern>(p: &'pattern str) -> Parser<'input, &'input str>
where
    'pattern: 'input,
{
    Parser {
        inner: Box::new(move |input: &'input str, state| {
            if input[state.current..].starts_with(p) {
                Ok((
                    &input[state.current..state.current + p.len()],
                    State {
                        current: state.current + p.len(),
                    },
                ))
            } else {
                Err(ParserError::NoParse(state.current))
            }
        }),
    }
}

fn or<'input, R: 'input>(first: Parser<'input, R>, second: Parser<'input, R>) -> Parser<'input, R> {
    Parser {
        inner: Box::new(
            move |input: &'input str, state| match (first.inner)(input, state) {
                Ok(result) => Ok(result),
                Err(_) => (second.inner)(input, state),
            },
        ),
    }
}

fn take_while<'input>(pred: impl Fn(char) -> bool + 'input) -> Parser<'input, &'input str> {
    Parser {
        inner: Box::new(move |input: &str, state| {
            let mut current = state.current;
            while current < input.len() && pred(input.chars().nth(current).unwrap()) {
                current += 1;
            }
            Ok((&input[state.current..current], State { current }))
        }),
    }
}

fn bind<'input, R: 'input, RR: 'input>(
    p: Parser<'input, R>,
    f: impl Fn(R) -> Parser<'input, RR> + 'input,
) -> Parser<'input, RR> {
    Parser {
        inner: Box::new(move |input: &'input str, state| {
            let (result, new_state) = (p.inner)(input, state)?;
            (f(result).inner)(input, new_state)
        }),
    }
}

fn pure<'input, R: 'input>(value: R) -> Parser<'input, R> {
    Parser {
        inner: Box::new(move |_: &'input str, state| Ok((value, state))),
    }
}

fn pure_fail<'input, R: 'input>(unwind: Option<usize>) -> Parser<'input, R> {
    Parser {
        inner: Box::new(move |_: &'input str, state| {
            Err(ParserError::NoParse(
                unwind.map_or(state.current, |u| state.current - u),
            ))
        }),
    }
}

fn string<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(pattern("\""), |_: &str| {
        bind(take_while(|c| c != '"'), |s| {
            bind(pattern("\""), move |_: &str| pure(JsonValue::String(s)))
        })
    })
}

fn merge_two_consecutive_strs<'input>(s1: &'input str, s2: &'input str) -> &'input str {
    unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(s1.as_ptr(), s1.len() + s2.len()))
    }
}

fn whole_part_number<'input>() -> Parser<'input, &'input str> {
    bind(or(pattern("-"), pattern("")), |sign| {
        bind(take_while(|c| c.is_digit(10)), |digits| {
            match digits.len() {
                0 => pure_fail(Some(sign.len())),
                1 => pure(merge_two_consecutive_strs(sign, digits)),
                other if digits.chars().nth(0).unwrap() == '0' => {
                    pure_fail(Some(other + sign.len()))
                }
                _ => pure(merge_two_consecutive_strs(sign, digits)),
            }
        })
    })
}

fn decimal_part_number<'input>() -> Parser<'input, &'input str> {
    bind(pattern("."), |dot: &str| {
        bind(take_while(|c| c.is_digit(10)), |digits| {
            pure(merge_two_consecutive_strs(dot, digits))
        })
    })
}

fn optional<'input, R: 'input>(parser: Parser<'input, R>) -> Parser<'input, Option<R>> {
    Parser {
        inner: Box::new(
            move |input: &'input str, state| match (parser.inner)(input, state) {
                Ok((result, new_state)) => Ok((Some(result), new_state)),
                Err(_) => Ok((None, state)),
            },
        ),
    }
}

fn number<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(whole_part_number(), |whole_part| {
        bind(
            optional(decimal_part_number()),
            move |decimal_part| match decimal_part {
                Some(decimal_part) => pure(JsonValue::Number(
                    merge_two_consecutive_strs(whole_part, decimal_part)
                        .parse::<f64>()
                        .unwrap(),
                )),
                None => pure(JsonValue::Number(whole_part.parse::<f64>().unwrap())),
            },
        )
    })
}

fn boolean<'input>() -> Parser<'input, JsonValue<'input>> {
    or(
        bind(pattern("true"), |_| pure(JsonValue::Boolean(true))),
        bind(pattern("false"), |_| pure(JsonValue::Boolean(false))),
    )
}

fn null<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(pattern("null"), |_| pure(JsonValue::Null))
}

#[derive(Debug, Clone, PartialEq)]
enum JsonValue<'input> {
    String(&'input str),
    Number(f64),
    Object(std::collections::HashMap<&'input str, JsonValue<'input>>),
    List(Vec<JsonValue<'input>>),
    Boolean(bool),
    Null,
}

mod tests {
    use super::*;

    // test the pattern function
    #[test]
    fn test_pattern() {
        let parser = pattern("hello");
        let input = "hello world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 5 }));
    }

    // test the or function
    #[test]
    fn test_or() {
        let parser = or(pattern("hello"), pattern("world"));
        let input = "world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("world", State { current: 5 }));
    }

    // test the take_while function
    #[test]
    fn test_take_while() {
        let parser = take_while(|c| c.is_alphabetic());
        let input = "hello world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 5 }));
    }

    // test the pure function
    #[test]
    fn test_pure() {
        let parser = pure("hello");
        let input = "world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 0 }));
    }

    // test the bind parser using the pure parser
    #[test]
    fn test_bind() {
        let parser = bind(pattern("hello"), |s| pure(s.to_uppercase()));
        let input = "hello world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("HELLO".to_string(), State { current: 5 }));
    }

    // test the string parser
    #[test]
    fn test_string() {
        let parser = string();
        let input = "\"hello\"";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::String("hello"), State { current: 7 }));
    }

    // test the number parser
    #[test]
    fn test_number() {
        let parser = number();
        let input = "123";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Number(123.0), State { current: 3 }));

        let parser = number();
        let input = "-123";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Number(-123.0), State { current: 4 }));

        let parser = number();
        let input = "-00000000000001";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));
    }

    // test the pure fail parser
    #[test]
    fn test_pure_fail() {
        let parser = pure_fail::<i64>(None);
        let input = "hello world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));

        let parser = pure_fail::<i64>(Some(2));
        let input = "hello world";
        let state = State { current: 2 };
        let result = (parser.inner)(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));
    }

    // test the whole part number parser
    #[test]
    fn test_whole_part_number() {
        let parser = whole_part_number();
        let input = "123";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("123", State { current: 3 }));

        let parser = whole_part_number();
        let input = "-123";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("-123", State { current: 4 }));

        let parser = whole_part_number();
        let input = "0";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("0", State { current: 1 }));

        let parser = whole_part_number();
        let input = "-00000000000001";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));
    }

    // test the decimal part number parser
    #[test]
    fn test_decimal_part_number() {
        let parser = decimal_part_number();
        let input = ".123";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (".123", State { current: 4 }));

        let parser = decimal_part_number();
        let input = ".00000000000001";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (".00000000000001", State { current: 15 }));
    }

    // test the boolean parser
    #[test]
    fn test_boolean() {
        let parser = boolean();
        let input = "true";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Boolean(true), State { current: 4 }));

        let parser = boolean();
        let input = "false";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Boolean(false), State { current: 5 }));
    }

    // test the null parser
    #[test]
    fn test_null() {
        let parser = null();
        let input = "null";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Null, State { current: 4 }));
    }
}
