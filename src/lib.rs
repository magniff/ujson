#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct State {
    current: usize,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
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

fn string<'input>() -> Parser<'input, &'input str> {
    bind(pattern("\""), |_: &str| {
        bind(take_while(|c| c != '"'), |s| {
            bind(pattern("\""), move |_: &str| pure(s))
        })
    })
}

fn number<'input>() -> Parser<'input, i64> {
    bind(or(pattern("-"), pattern("")), |sign| {
        bind(take_while(|c| c.is_digit(10)), move |digits| {
            pure(
                unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        sign.as_ptr(),
                        sign.len() + digits.len(),
                    ))
                }
                .parse::<i64>()
                .unwrap(),
            )
        })
    })
}

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
    assert_eq!(result, ("hello", State { current: 7 }));
}

// test the number parser
#[test]
fn test_number() {
    let parser = number();
    let input = "123";
    let state = State { current: 0 };
    let result = (parser.inner)(input, state).unwrap();
    assert_eq!(result, (123, State { current: 3 }));

    let parser = number();
    let input = "-123";
    let state = State { current: 0 };
    let result = (parser.inner)(input, state).unwrap();
    assert_eq!(result, (-123, State { current: 4 }));
}
