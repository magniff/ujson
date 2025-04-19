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

trait Parser<'input, R> {
    fn parse(&self, input: &'input str, state: State) -> Result<(R, State), ParserError>;
}

impl<'input, R, F> Parser<'input, R> for F
where
    F: Fn(&'input str, State) -> Result<(R, State), ParserError> + 'input,
{
    #[inline(always)]
    fn parse(&self, input: &'input str, state: State) -> Result<(R, State), ParserError> {
        self(input, state)
    }
}

fn pat<'input, 'pattern>(p: &'pattern str) -> impl Parser<'input, &'input str>
where
    'pattern: 'input,
{
    #[inline]
    move |input: &'input str, state: State| {
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
    }
}

fn pat_ws<'input, 'pattern>(p: &'pattern str) -> impl Parser<'input, &'input str>
where
    'pattern: 'input,
{
    bind(
        take_while(|c| c.is_whitespace()),
        #[inline]
        move |_: &str| {
            bind(pat(p), move |s| {
                bind(take_while(|c| c.is_whitespace()), move |_: &str| success(s))
            })
        },
    )
}

fn or<'input, R: 'input>(
    first: impl Parser<'input, R> + 'input,
    second: impl Parser<'input, R> + 'input,
) -> impl Parser<'input, R> + 'input {
    #[inline]
    move |input: &'input str, state| match first.parse(input, state) {
        Ok(result) => Ok(result),
        Err(_) => second.parse(input, state),
    }
}

fn take_while<'input>(
    pred: impl Fn(char) -> bool + 'input,
) -> impl Parser<'input, &'input str> + 'input {
    #[inline]
    move |input: &'input str, state: State| {
        let end = input[state.current..]
            .char_indices()
            .take_while(|(_, c)| pred(*c))
            .last()
            .map_or(state.current, |(index, _)| state.current + index + 1);
        Ok((&input[state.current..end], State { current: end }))
    }
}

fn bind<'input, R: 'input, RR: 'input, P>(
    p: impl Parser<'input, R> + 'input,
    f: impl Fn(R) -> P + 'input,
) -> impl Parser<'input, RR>
where
    P: Parser<'input, RR> + 'input,
{
    #[inline]
    move |input: &'input str, state| {
        let (result, new_state) = p.parse(input, state)?;
        f(result).parse(input, new_state)
    }
}

fn success<'input, R: Clone + 'input>(value: R) -> impl Parser<'input, R> {
    #[inline]
    move |_: &'input str, state| Ok((value.clone(), state))
}

fn fail<'input, R: 'input>(unwind: Option<usize>) -> impl Parser<'input, R> {
    #[inline]
    move |_: &'input str, state: State| {
        Err(ParserError::NoParse(
            unwind.map_or(state.current, |u| state.current - u),
        ))
    }
}

fn string<'input>() -> impl Parser<'input, JsonValue<'input>> {
    bind(
        pat("\""),
        #[inline]
        |_: &str| {
            bind(
                take_while(|c| c != '"'),
                #[inline]
                |s| {
                    bind(
                        pat("\""),
                        #[inline]
                        move |_: &str| success(JsonValue::String(s)),
                    )
                },
            )
        },
    )
}

fn merge_two_consecutive_strs<'input>(s1: &'input str, s2: &'input str) -> &'input str {
    unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(s1.as_ptr(), s1.len() + s2.len()))
    }
}

enum Either<A, B, C, D> {
    A(A),
    B(B),
    C(C),
    D(D),
}

impl<'input, A, B, C, D, R> Parser<'input, R> for Either<A, B, C, D>
where
    A: Parser<'input, R>,
    B: Parser<'input, R>,
    C: Parser<'input, R>,
    D: Parser<'input, R>,
{
    #[inline(always)]
    fn parse(&self, input: &'input str, state: State) -> Result<(R, State), ParserError> {
        match self {
            Either::A(a) => a.parse(input, state),
            Either::B(b) => b.parse(input, state),
            Either::C(c) => c.parse(input, state),
            Either::D(d) => d.parse(input, state),
        }
    }
}

fn whole_part_number<'input>() -> impl Parser<'input, &'input str> {
    bind(
        or(pat("-"), pat("")),
        #[inline]
        |sign| {
            bind(
                take_while(|c| c.is_digit(10)),
                #[inline]
                |digits| match digits.len() {
                    0 => Either::A(fail(Some(sign.len()))),
                    1 => Either::B(success(merge_two_consecutive_strs(sign, digits))),
                    other if digits.chars().nth(0).unwrap() == '0' => {
                        Either::C(fail(Some(other + sign.len())))
                    }
                    _ => Either::D(success(merge_two_consecutive_strs(sign, digits))),
                },
            )
        },
    )
}

fn decimal_part_number<'input>() -> impl Parser<'input, &'input str> {
    bind(pat("."), |dot: &str| {
        bind(take_while(|c| c.is_digit(10)), |digits| {
            success(merge_two_consecutive_strs(dot, digits))
        })
    })
}

fn optional<'input, R: 'input>(
    parser: impl Parser<'input, R> + 'input,
) -> impl Parser<'input, Option<R>> {
    #[inline]
    move |input: &'input str, state| match parser.parse(input, state) {
        Ok((result, new_state)) => Ok((Some(result), new_state)),
        Err(_) => Ok((None, state)),
    }
}

fn spaced_by<'input, R: 'input, S: 'input>(
    parser: impl Parser<'input, R> + 'input,
    spacer: impl Parser<'input, S> + 'input,
) -> impl Parser<'input, Vec<R>> + 'input {
    #[inline]
    move |input: &'input str, state| {
        let mut results = Vec::new();
        let (first_result, mut state) = parser.parse(input, state)?;
        results.push(first_result);

        while let Ok((_, new_state)) = spacer.parse(input, state) {
            match parser.parse(input, new_state) {
                Ok((result, new_state)) => {
                    results.push(result);
                    state = new_state;
                }
                Err(_) => break,
            }
        }

        Ok((results, state))
    }
}

fn json_value<'input>() -> impl Parser<'input, JsonValue<'input>> {
    move |input: &'input str, state| {
        or(
            string(),
            or(
                number(),
                or(object(), or(list(), or(boolean(), or(null(), fail(None))))),
            ),
        )
        .parse(input, state)
    }
}

fn key_value_pair<'input>() -> impl Parser<'input, (&'input str, JsonValue<'input>)> {
    bind(string(), move |key| {
        bind(pat_ws(":"), move |_: &str| {
            let JsonValue::String(key) = key else {
                panic!("internal error in key_value_pair, key is not a string")
            };
            bind(json_value(), move |value| success((key, value)))
        })
    })
}

fn object<'input>() -> impl Parser<'input, JsonValue<'input>> {
    bind(pat_ws("{"), |_: &str| {
        bind(
            spaced_by(key_value_pair(), pat_ws(",")),
            move |key_value_pairs| {
                let key_value_pairs: std::rc::Rc<_> = std::rc::Rc::from(key_value_pairs);
                bind(pat_ws("}"), move |_: &str| {
                    success(JsonValue::Object(std::rc::Rc::clone(&key_value_pairs)))
                })
            },
        )
    })
}

fn list<'input>() -> impl Parser<'input, JsonValue<'input>> {
    bind(pat_ws("["), |_: &str| {
        bind(spaced_by(json_value(), pat_ws(",")), move |values| {
            let values: std::rc::Rc<_> = std::rc::Rc::from(values);
            bind(pat_ws("]"), move |_: &str| {
                success(JsonValue::List(std::rc::Rc::clone(&values)))
            })
        })
    })
}

fn number<'input>() -> impl Parser<'input, JsonValue<'input>> {
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

fn boolean<'input>() -> impl Parser<'input, JsonValue<'input>> {
    or(
        bind(
            pat("true"),
            #[inline]
            |_| success(JsonValue::Boolean(true)),
        ),
        bind(
            pat("false"),
            #[inline]
            |_| success(JsonValue::Boolean(false)),
        ),
    )
}

fn null<'input>() -> impl Parser<'input, JsonValue<'input>> {
    bind(pat("null"), |_| success(JsonValue::Null))
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue<'input> {
    String(&'input str),
    Number(f64),
    Object(std::rc::Rc<[(&'input str, JsonValue<'input>)]>),
    List(std::rc::Rc<[JsonValue<'input>]>),
    Boolean(bool),
    Null,
}

pub fn from_str<'input>(input: &'input str) -> Result<JsonValue<'input>, ParserError> {
    let state = State { current: 0 };
    let (result, state) = json_value().parse(input, state)?;
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
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 5 }));
    }

    // test the or function
    #[test]
    fn test_or() {
        let parser = or(pat("hello"), pat("world"));
        let input = "world";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("world", State { current: 5 }));
    }

    // test the take_while function
    #[test]
    fn test_take_while() {
        let parser = take_while(|c| c.is_alphabetic());
        let input = "hello world";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 5 }));
    }

    // test the pure function
    #[test]
    fn test_pure() {
        let parser = success("hello");
        let input = "world";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("hello", State { current: 0 }));
    }

    // test the bind parser using the pure parser
    #[test]
    fn test_bind() {
        let parser = bind(pat("hello"), |s| success(s.to_uppercase()));
        let input = "hello world";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("HELLO".to_string(), State { current: 5 }));
    }

    // test the string parser
    #[test]
    fn test_string() {
        let parser = string();
        let input = "\"hello\"";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::String("hello"), State { current: 7 }));
    }

    // test the number parser
    #[test]
    fn test_number() {
        let parser = number();
        let input = "123";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Number(123.0), State { current: 3 }));

        let parser = number();
        let input = "-123";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Number(-123.0), State { current: 4 }));

        let parser = number();
        let input = "-00000000000001";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));
    }

    // test the pure fail parser
    #[test]
    fn test_pure_fail() {
        let parser = fail::<i64>(None);
        let input = "hello world";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));

        let parser = fail::<i64>(Some(2));
        let input = "hello world";
        let state = State { current: 2 };
        let result = parser.parse(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));
    }

    // test the whole part number parser
    #[test]
    fn test_whole_part_number() {
        let parser = whole_part_number();
        let input = "123";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("123", State { current: 3 }));

        let parser = whole_part_number();
        let input = "-123";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("-123", State { current: 4 }));

        let parser = whole_part_number();
        let input = "0";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, ("0", State { current: 1 }));

        let parser = whole_part_number();
        let input = "-00000000000001";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap_err();
        assert_eq!(result, ParserError::NoParse(0));
    }

    // test the decimal part number parser
    #[test]
    fn test_decimal_part_number() {
        let parser = decimal_part_number();
        let input = ".123";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (".123", State { current: 4 }));

        let parser = decimal_part_number();
        let input = ".00000000000001";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (".00000000000001", State { current: 15 }));
    }

    // test the boolean parser
    #[test]
    fn test_boolean() {
        let parser = boolean();
        let input = "true";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Boolean(true), State { current: 4 }));

        let parser = boolean();
        let input = "false";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Boolean(false), State { current: 5 }));
    }

    // test the null parser
    #[test]
    fn test_null() {
        let parser = null();
        let input = "null";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Null, State { current: 4 }));
    }

    // test the json value parser
    #[test]
    fn test_json_value() {
        let parser = json_value();
        let input = "\"hello\"";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::String("hello"), State { current: 7 }));

        let parser = json_value();
        let input = "123";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Number(123.0), State { current: 3 }));

        let parser = json_value();
        let input = "true";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Boolean(true), State { current: 4 }));

        let parser = json_value();
        let input = "null";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(result, (JsonValue::Null, State { current: 4 }));

        let parser = json_value();
        let input = "[1, 2, 3]";
        let state = State { current: 0 };
        let result = parser.parse(input, state).unwrap();
        assert_eq!(
            result,
            (
                JsonValue::List(std::rc::Rc::from(vec![
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
        let result = parser.parse(input, state).unwrap();
        assert_eq!(
            result,
            (
                JsonValue::Object(std::rc::Rc::from(vec![("key", JsonValue::String("value"))])),
                State {
                    current: input.len()
                }
            )
        );
    }
}
