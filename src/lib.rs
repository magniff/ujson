#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct State {
    current: usize,
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum ParserError {
    #[error("Parse error at position {0}")]
    NoParse(usize),
}

pub struct Parser<'input, R> {
    inner: Box<dyn Fn(&'input str, State) -> Result<(R, State), ParserError> + 'input>,
}

fn pat<'input, 'pattern>(p: &'pattern str) -> Parser<'input, &'input str>
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

fn pat_ws<'input, 'pattern>(p: &'pattern str) -> Parser<'input, &'input str>
where
    'pattern: 'input,
{
    bind(take_while(|c| c.is_whitespace()), move |_: &str| {
        bind(pat(p), move |s| {
            bind(take_while(|c| c.is_whitespace()), move |_: &str| success(s))
        })
    })
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
            let end = input[state.current..]
                .char_indices()
                .take_while(|(_, c)| pred(*c))
                .last()
                .map_or(state.current, |(index, _)| state.current + index + 1);
            Ok((&input[state.current..end], State { current: end }))
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

fn success<'input, R: Clone + 'input>(value: R) -> Parser<'input, R> {
    Parser {
        inner: Box::new(move |_: &'input str, state| Ok((value.clone(), state))),
    }
}

fn fail<'input, R: 'input>(unwind: Option<usize>) -> Parser<'input, R> {
    Parser {
        inner: Box::new(move |_: &'input str, state| {
            Err(ParserError::NoParse(
                unwind.map_or(state.current, |u| state.current - u),
            ))
        }),
    }
}

fn string<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(pat("\""), |_: &str| {
        bind(take_while(|c| c != '"'), |s| {
            bind(pat("\""), move |_: &str| success(JsonValue::String(s)))
        })
    })
}

fn merge_two_consecutive_strs<'input>(s1: &'input str, s2: &'input str) -> &'input str {
    unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(s1.as_ptr(), s1.len() + s2.len()))
    }
}

fn whole_part_number<'input>() -> Parser<'input, &'input str> {
    bind(or(pat("-"), pat("")), |sign| {
        bind(take_while(|c| c.is_digit(10)), |digits| {
            match digits.len() {
                0 => fail(Some(sign.len())),
                1 => success(merge_two_consecutive_strs(sign, digits)),
                other if digits.chars().nth(0).unwrap() == '0' => fail(Some(other + sign.len())),
                _ => success(merge_two_consecutive_strs(sign, digits)),
            }
        })
    })
}

fn decimal_part_number<'input>() -> Parser<'input, &'input str> {
    bind(pat("."), |dot: &str| {
        bind(take_while(|c| c.is_digit(10)), |digits| {
            success(merge_two_consecutive_strs(dot, digits))
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

fn spaced_by<'input, R: 'input, S: 'input>(
    parser: Parser<'input, R>,
    spacer: Parser<'input, S>,
) -> Parser<'input, Vec<R>> {
    Parser {
        inner: Box::new(move |input: &'input str, state| {
            let mut results = Vec::with_capacity(8);
            let (first_result, mut state) = (parser.inner)(input, state)?;
            results.push(first_result);

            while let Ok((_, new_state)) = (spacer.inner)(input, state) {
                match (parser.inner)(input, new_state) {
                    Ok((result, new_state)) => {
                        results.push(result);
                        state = new_state;
                    }
                    Err(_) => break,
                }
            }

            Ok((results, state))
        }),
    }
}

fn json_value<'input>() -> Parser<'input, JsonValue<'input>> {
    or(
        string(),
        or(
            number(),
            or(object(), or(list(), or(boolean(), or(null(), fail(None))))),
        ),
    )
}

fn key_value_pair<'input>() -> Parser<'input, (&'input str, JsonValue<'input>)> {
    bind(string(), move |key| {
        bind(pat_ws(":"), move |_: &str| {
            let JsonValue::String(key) = key else {
                panic!("internal error in key_value_pair, key is not a string")
            };
            bind(json_value(), move |value| success((key, value)))
        })
    })
}

fn object<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(pat_ws("{"), |_: &str| {
        bind(
            spaced_by(key_value_pair(), pat_ws(",")),
            move |key_value_pairs| {
                let key_value_pairs = std::rc::Rc::new(key_value_pairs);
                bind(pat_ws("}"), move |_: &str| {
                    success(JsonValue::Object(key_value_pairs.clone()))
                })
            },
        )
    })
}

fn list<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(pat_ws("["), |_: &str| {
        bind(spaced_by(json_value(), pat_ws(",")), move |values| {
            let values = std::rc::Rc::new(values);
            bind(pat_ws("]"), move |_: &str| {
                success(JsonValue::List(values.clone()))
            })
        })
    })
}

fn number<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(whole_part_number(), |whole_part| {
        bind(
            optional(decimal_part_number()),
            move |decimal_part| match decimal_part {
                Some(decimal_part) => success(JsonValue::Number(
                    merge_two_consecutive_strs(whole_part, decimal_part)
                        .parse::<f64>()
                        .unwrap(),
                )),
                None => success(JsonValue::Number(whole_part.parse::<f64>().unwrap())),
            },
        )
    })
}

fn boolean<'input>() -> Parser<'input, JsonValue<'input>> {
    or(
        bind(pat("true"), |_| success(JsonValue::Boolean(true))),
        bind(pat("false"), |_| success(JsonValue::Boolean(false))),
    )
}

fn null<'input>() -> Parser<'input, JsonValue<'input>> {
    bind(pat("null"), |_| success(JsonValue::Null))
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue<'input> {
    String(&'input str),
    Number(f64),
    Object(std::rc::Rc<Vec<(&'input str, JsonValue<'input>)>>),
    List(std::rc::Rc<Vec<JsonValue<'input>>>),
    Boolean(bool),
    Null,
}

pub fn from_str<'input>(input: &'input str) -> Result<JsonValue<'input>, ParserError> {
    let state = State { current: 0 };
    let (result, state) = (json_value().inner)(input, state)?;
    if state.current == input.len() {
        Ok(result)
    } else {
        Err(ParserError::NoParse(state.current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // test the pattern function
    #[test]
    fn test_pattern() {
        let parser = pat("hello");
        let input = "hello world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 5 }));
    }

    // test the or function
    #[test]
    fn test_or() {
        let parser = or(pat("hello"), pat("world"));
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
        let parser = success("hello");
        let input = "world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 0 }));
    }

    // test the bind parser using the pure parser
    #[test]
    fn test_bind() {
        let parser = bind(pat("hello"), |s| success(s.to_uppercase()));
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
        let parser = fail::<i64>(None);
        let input = "hello world";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));

        let parser = fail::<i64>(Some(2));
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

    // test the json value parser
    #[test]
    fn test_json_value() {
        let parser = json_value();
        let input = "\"hello\"";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::String("hello"), State { current: 7 }));

        let parser = json_value();
        let input = "123";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Number(123.0), State { current: 3 }));

        let parser = json_value();
        let input = "true";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Boolean(true), State { current: 4 }));

        let parser = json_value();
        let input = "null";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(result, (JsonValue::Null, State { current: 4 }));

        let parser = json_value();
        let input = "[1, 2, 3]";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(
            result,
            (
                JsonValue::List(std::rc::Rc::new(vec![
                    JsonValue::Number(1.0),
                    JsonValue::Number(2.0),
                    JsonValue::Number(3.0)
                ])),
                State {
                    current: input.len()
                }
            )
        );

        let parser = json_value();
        let input = "{\"key\": \"value\"}";
        let state = State { current: 0 };
        let result = (parser.inner)(input, state).unwrap();
        assert_eq!(
            result,
            (
                JsonValue::Object(std::rc::Rc::new(vec![("key", JsonValue::String("value"))])),
                State {
                    current: input.len()
                }
            )
        );
    }
}
