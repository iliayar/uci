use nom::{
    self,
    branch::alt,
    bytes::complete::{escaped, is_not, tag, take_while},
    character::complete::{char, digit1, one_of, satisfy},
    combinator::{cut, map, map_res, opt, peek, recognize, value},
    error::context,
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{pair, preceded, terminated, tuple},
    AsChar, IResult,
};

type Input<'a> = &'a str;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LValue<'a> {
    String(&'a str),
    QuoteString(&'a str),
    Integer(i64),
    Boolean(bool),
    Null,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LPathSegment<'a> {
    Expression(LExpression<'a>),
    String(&'a str),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LFsPathSegment<'a> {
    String(&'a str),
    Expression(LExpression<'a>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LFsPathType {
    Absolute,
    Relative,
    FromHome,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LExpression<'a> {
    Value(LValue<'a>),
    Path(Vec<LPathSegment<'a>>, Box<PathScope<'a>>),
    FsPath(Vec<LFsPathSegment<'a>>, LFsPathType),
    FunctionCall(&'a str, Vec<LExpression<'a>>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PathScope<'a> {
    Global,
    Expression(LExpression<'a>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct LFormatString<'a>(pub(crate) Vec<LFormatStringFragment<'a>>);

#[derive(Debug, PartialEq, Eq)]
pub enum LFormatStringFragment<'a> {
    Expression(LExpression<'a>),
    Raw(&'a str),
}

fn ws(input: Input) -> IResult<Input, ()> {
    let chars = " \t\n\r";
    nom::combinator::value((), take_while(move |c| chars.contains(c)))(input)
}

fn number(input: Input) -> IResult<Input, LValue> {
    let (input, neg) = map(opt(tag("-")), |v| v.is_some())(input)?;
    let (input, _) = ws(input)?;
    let (input, num) = map_res(digit1, |n: &str| n.parse::<i64>())(input)?;
    Ok((input, LValue::Integer(if neg { -num } else { num })))
}

fn string_escaped(input: Input) -> IResult<Input, &str> {
    let escaped_string = escaped(is_not("\"\\"), '\\', one_of("\"rn\\"));
    context(
        "string",
        recognize(preceded(
            char('\"'),
            cut(terminated(escaped_string, char('\"'))),
        )),
    )(input)
}

fn string_raw(input: Input) -> IResult<Input, &str> {
    context(
        "raw_string",
        preceded(char('\''), cut(terminated(is_not("\'"), char('\'')))),
    )(input)
}

fn string(input: Input) -> IResult<Input, LValue> {
    alt((
        map(string_escaped, LValue::String),
        map(string_raw, LValue::QuoteString),
    ))(input)
}

fn boolean(input: Input) -> IResult<Input, LValue> {
    map(
        alt((value(true, tag("true")), value(false, tag("false")))),
        LValue::Boolean,
    )(input)
}

fn null(input: Input) -> IResult<Input, LValue> {
    value(LValue::Null, tag("null"))(input)
}

fn lvalue(input: Input) -> IResult<Input, LValue> {
    alt((number, string, boolean, null))(input)
}

fn generic_identifier<'a>(
    is_identifier_first: impl Fn(char) -> bool,
    is_identifier: impl Fn(char) -> bool,
) -> impl FnMut(Input<'a>) -> IResult<Input<'a>, &'a str> {
    recognize(pair(
        satisfy(is_identifier_first),
        many0(satisfy(is_identifier)),
    ))
}

fn identifier(input: Input) -> IResult<Input, &str> {
    generic_identifier(
        |c| c.is_alpha() || "_".contains(c),
        |c| c.is_alphanumeric() || "_-".contains(c),
    )(input)
}

fn efs_path(input: Input) -> IResult<Input, LExpression> {
    fn fs_path_segment_name(input: Input) -> IResult<Input, &str> {
        let pred = |c: char| c.is_alphanumeric() || ".-_".contains(c);
        generic_identifier(pred, pred)(input)
    }

    fn fs_path_segment(input: Input) -> IResult<Input, LFsPathSegment> {
        alt((
            map(fs_path_segment_name, LFsPathSegment::String),
            map(lexpression, LFsPathSegment::Expression),
        ))(input)
    }

    let path_type = alt((
        value(LFsPathType::Absolute, tag("/")),
        value(LFsPathType::Relative, tag("./")),
        value(LFsPathType::FromHome, tag("~/")),
    ));

    context(
        "path",
        map(
            tuple((path_type, cut(separated_list1(tag("/"), fs_path_segment)))),
            |(path_type, path)| LExpression::FsPath(path, path_type),
        ),
    )(input)
}

fn path_segment(input: Input) -> IResult<Input, LPathSegment> {
    alt((
        map(identifier, LPathSegment::String),
        map(lexpression, LPathSegment::Expression),
    ))(input)
}

fn epath(input: Input) -> IResult<Input, LExpression> {
    // TODO: Make possible syntax func(args...).key
    // Now there is a collision: func is prefix of path and path is
    // prefix of func
    let scope = alt((
        map(terminated(lexpression, char('.')), PathScope::Expression),
        value(PathScope::Global, peek(identifier)),
    ));

    map(
        tuple((scope, separated_list1(char('.'), path_segment))),
        |(scope, path)| LExpression::Path(path, Box::new(scope)),
    )(input)
}

pub fn lexpression(input: Input) -> IResult<Input, LExpression> {
    context(
        "expression",
        preceded(
            tag("${"),
            cut(terminated(
                preceded(ws, inner_expression),
                preceded(ws, char('}')),
            )),
        ),
    )(input)
}

pub fn inner_expression(input: Input) -> IResult<Input, LExpression> {
    alt((
        map(lvalue, LExpression::Value),
        efunction_call,
        epath,
        efs_path,
        lexpression,
    ))(input)
}

fn efunction_call(input: Input) -> IResult<Input, LExpression> {
    let function_identifier = generic_identifier(
        |c| c.is_alpha() || "_".contains(c),
        |c| c.is_alphanumeric() || "_-".contains(c),
    );

    let arg = alt((lexpression, inner_expression));

    map(
        tuple((
            function_identifier,
            preceded(
                preceded(ws, tag("(")),
                cut(terminated(
                    separated_list0(preceded(ws, char(',')), preceded(ws, arg)),
                    preceded(ws, tag(")")),
                )),
            ),
        )),
        |(func, args)| LExpression::FunctionCall(func, args),
    )(input)
}

fn format_string<'a>(
    string_char: impl Fn(char) -> bool,
) -> impl FnMut(Input<'a>) -> IResult<Input<'a>, LFormatString<'a>> {
    let raw_string_char = move |c: char| c != '$' && string_char(c);

    let raw_string_regular = map(
        recognize(many1(satisfy(raw_string_char))),
        LFormatStringFragment::Raw,
    );
    let escaped_expr = map(preceded(char('$'), tag("${")), LFormatStringFragment::Raw);
    let dollar_sign = map(tag("$"), LFormatStringFragment::Raw);
    let raw_string = alt((raw_string_regular, escaped_expr, dollar_sign));

    let expr_fragment = map(lexpression, LFormatStringFragment::Expression);
    let format_string = alt((expr_fragment, raw_string));

    map(many0(format_string), LFormatString)
}

pub fn lformat_string(input: Input) -> IResult<Input, LFormatString> {
    format_string(|_| true)(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_ok() {
        assert_eq!(number("123"), Ok(("", LValue::Integer(123))));
        assert_eq!(number("-123"), Ok(("", LValue::Integer(-123))));
        assert_eq!(number("0"), Ok(("", LValue::Integer(0))));
        assert_eq!(number("- 2"), Ok(("", LValue::Integer(-2))));
    }

    #[test]
    fn test_number_err() {
        assert!(matches!(number("a"), Err(nom::Err::Error(_))));
        assert!(matches!(number("-b"), Err(nom::Err::Error(_))));
        assert!(matches!(number(""), Err(nom::Err::Error(_))));
    }

    #[test]
    fn test_string() {
        assert_eq!(string(r#""12?$3""#), Ok(("", LValue::String("\"12?$3\""))));
        assert_eq!(
            string(r#""1\\2\n3""#),
            Ok(("", LValue::String("\"1\\\\2\\n3\"")))
        );
        assert_eq!(
            string(r#"'1\\2\n\p3'"#),
            Ok(("", LValue::QuoteString("1\\\\2\\n\\p3")))
        );
    }

    #[test]
    fn test_number_failure() {
        assert!(matches!(string(r#""123"#), Err(nom::Err::Failure(_))));
        assert!(matches!(string(r#""12\p3""#), Err(nom::Err::Failure(_))));
        assert!(matches!(string(r#"'12\g3""#), Err(nom::Err::Failure(_))));
        assert!(matches!(string(r#""12\g3'"#), Err(nom::Err::Failure(_))));
        assert!(matches!(string(r#"'12\g3"#), Err(nom::Err::Failure(_))));
    }

    #[test]
    fn test_boolean() {
        assert_eq!(boolean("true"), Ok(("", LValue::Boolean(true))));
        assert_eq!(boolean("false"), Ok(("", LValue::Boolean(false))));
    }

    #[test]
    fn test_boolean_error() {
        assert!(matches!(boolean("tru"), Err(nom::Err::Error(_))));
        assert!(matches!(boolean("fase"), Err(nom::Err::Error(_))));
    }

    #[test]
    fn test_value() {
        assert_eq!(lvalue("true"), Ok(("", LValue::Boolean(true))));
        assert_eq!(lvalue(r#""123""#), Ok(("", LValue::String("\"123\""))));
        assert_eq!(lvalue(r#"'123'"#), Ok(("", LValue::QuoteString("123"))));
        assert_eq!(lvalue("-123"), Ok(("", LValue::Integer(-123))));
        assert_eq!(lvalue("0"), Ok(("", LValue::Integer(0))));
    }

    #[test]
    fn test_path_segment() {
        assert_eq!(
            path_segment("aboba"),
            Ok(("", LPathSegment::String("aboba")))
        );
        assert_eq!(
            path_segment("${1}"),
            Ok((
                "",
                LPathSegment::Expression(LExpression::Value(LValue::Integer(1)))
            ))
        );
    }

    #[test]
    fn test_path() {
        assert_eq!(
            epath("aboba.foo.bar"),
            Ok((
                "",
                LExpression::Path(
                    vec![
                        LPathSegment::String("aboba"),
                        LPathSegment::String("foo"),
                        LPathSegment::String("bar")
                    ],
                    Box::new(PathScope::Global)
                )
            ))
        );
        assert_eq!(
            epath("aboba.${\"123\"}.${0}"),
            Ok((
                "",
                LExpression::Path(
                    vec![
                        LPathSegment::String("aboba"),
                        LPathSegment::Expression(LExpression::Value(LValue::String("\"123\""))),
                        LPathSegment::Expression(LExpression::Value(LValue::Integer(0))),
                    ],
                    Box::new(PathScope::Global)
                )
            ))
        );
    }

    #[test]
    fn test_fs_path() {
        assert_eq!(
            efs_path("./a/b/c"),
            Ok((
                "",
                LExpression::FsPath(
                    vec![
                        LFsPathSegment::String("a"),
                        LFsPathSegment::String("b"),
                        LFsPathSegment::String("c"),
                    ],
                    LFsPathType::Relative
                )
            ))
        );
        assert_eq!(
            efs_path("./a"),
            Ok((
                "",
                LExpression::FsPath(vec![LFsPathSegment::String("a"),], LFsPathType::Relative)
            ))
        );
        assert_eq!(
            efs_path("/b"),
            Ok((
                "",
                LExpression::FsPath(vec![LFsPathSegment::String("b"),], LFsPathType::Absolute)
            ))
        );
        assert_eq!(
            efs_path("~/.config"),
            Ok((
                "",
                LExpression::FsPath(vec![LFsPathSegment::String(".config"),], LFsPathType::FromHome)
            ))
        );
        assert_eq!(
            efs_path("/${root}/a"),
            Ok((
                "",
                LExpression::FsPath(
                    vec![
                        LFsPathSegment::Expression(LExpression::Path(
                            vec![LPathSegment::String("root")],
                            Box::new(PathScope::Global)
                        )),
                        LFsPathSegment::String("a"),
                    ],
                    LFsPathType::Absolute
                )
            ))
        );
    }

    #[test]
    fn test_function_call() {
        assert_eq!(
            efunction_call("concat(${\"a\"}, b)"),
            Ok((
                "",
                LExpression::FunctionCall(
                    "concat",
                    vec![
                        LExpression::Value(LValue::String("\"a\"")),
                        LExpression::Path(
                            vec![LPathSegment::String("b")],
                            Box::new(PathScope::Global)
                        ),
                    ],
                )
            ))
        );
        assert_eq!(
            efunction_call("fail()"),
            Ok(("", LExpression::FunctionCall("fail", vec![],)))
        );
    }

    #[test]
    fn test_format_string() {
        assert_eq!(
            lformat_string("test-${name}"),
            Ok((
                "",
                LFormatString(vec![
                    LFormatStringFragment::Raw("test-"),
                    LFormatStringFragment::Expression(LExpression::Path(
                        vec![LPathSegment::String("name")],
                        Box::new(PathScope::Global)
                    ))
                ]),
            ))
        );
        assert_eq!(
            lformat_string("string$with$dollars))"),
            Ok((
                "",
                LFormatString(vec![
                    LFormatStringFragment::Raw("string"),
                    LFormatStringFragment::Raw("$"),
                    LFormatStringFragment::Raw("with"),
                    LFormatStringFragment::Raw("$"),
                    LFormatStringFragment::Raw("dollars))"),
                ]),
            ))
        );
        assert_eq!(
            lformat_string("escaped-$${expr}-more-$$$${expr}"),
            Ok((
                "",
                LFormatString(vec![
                    LFormatStringFragment::Raw("escaped-"),
                    LFormatStringFragment::Raw("${"),
                    LFormatStringFragment::Raw("expr}-more-"),
                    LFormatStringFragment::Raw("$"),
                    LFormatStringFragment::Raw("$"),
                    LFormatStringFragment::Raw("${"),
                    LFormatStringFragment::Raw("expr}"),
                ]),
            ))
        );
    }

    #[test]
    fn test_expression() {
        assert_eq!(
            lexpression("${aboba}"),
            Ok((
                "",
                LExpression::Path(
                    vec![LPathSegment::String("aboba")],
                    Box::new(PathScope::Global)
                )
            ))
        );
        assert_eq!(
            lexpression("${true}"),
            Ok(("", LExpression::Value(LValue::Boolean(true))))
        );
        assert_eq!(
            lexpression("${- 2}"),
            Ok(("", LExpression::Value(LValue::Integer(-2))))
        );
        assert_eq!(
            lexpression("${${0}}"),
            Ok(("", LExpression::Value(LValue::Integer(0))))
        );
        assert_eq!(
            lexpression("${a1-23.${0}}"),
            Ok((
                "",
                LExpression::Path(
                    vec![
                        LPathSegment::String("a1-23"),
                        LPathSegment::Expression(LExpression::Value(LValue::Integer(0)))
                    ],
                    Box::new(PathScope::Global)
                )
            ))
        );
        assert_eq!(
            lexpression("${./a}"),
            Ok((
                "",
                LExpression::FsPath(vec![LFsPathSegment::String("a"),], LFsPathType::Relative)
            ))
        );
        assert_eq!(
            lexpression("${/${a}}"),
            Ok((
                "",
                LExpression::FsPath(
                    vec![LFsPathSegment::Expression(LExpression::Path(
                        vec![LPathSegment::String("a")],
                        Box::new(PathScope::Global)
                    )),],
                    LFsPathType::Absolute
                )
            ))
        );
        assert_eq!(
            lexpression("${if(true, ${a}, ${b})}"),
            Ok((
                "",
                LExpression::FunctionCall(
                    "if",
                    vec![
                        LExpression::Value(LValue::Boolean(true)),
                        LExpression::Path(
                            vec![LPathSegment::String("a")],
                            Box::new(PathScope::Global)
                        ),
                        LExpression::Path(
                            vec![LPathSegment::String("b")],
                            Box::new(PathScope::Global)
                        ),
                    ],
                )
            ))
        );
    }
}
