#![no_std]
#![feature(new_range_api)]

extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};
use chumsky::{
	error::Rich,
	extra,
	prelude::{choice, just, one_of, recursive},
	span::SimpleSpan,
	IterParser, Parser,
};

#[derive(Debug, Clone, PartialEq)]
#[repr(align(8))]
pub enum Expression<'src> {
	Integer(i64),
	String(String),
	Identifier(&'src str),
	LetIn {
		bindings: Vec<(Expression<'src>, Expression<'src>)>,
		body: Box<Expression<'src>>,
	},
	Add(Box<Expression<'src>>, Box<Expression<'src>>),
	Sub(Box<Expression<'src>>, Box<Expression<'src>>),
	Mul(Box<Expression<'src>>, Box<Expression<'src>>),
	Div(Box<Expression<'src>>, Box<Expression<'src>>),
	Paren(Box<Expression<'src>>),
	BinaryOperation(Box<Expression<'src>>, &'src str, Box<Expression<'src>>),
}

pub type Span = SimpleSpan<usize>;

pub fn expression<'src>(
) -> impl Parser<'src, &'src str, Expression<'src>, extra::Err<Rich<'src, char, Span>>>
{
	recursive(|expr| {
		let ident = chumsky::text::ident()
			.map(|i: &str| Expression::Identifier(i))
			.labelled("identifier");

		let integer = chumsky::text::int::<
			'src,
			&'src str,
			char,
			extra::Err<Rich<'src, char, Span>>,
		>(10)
		.to_slice()
		.from_str::<i64>()
		.unwrapped()
		.labelled("integer")
		.map(Expression::Integer);

		let value = choice((ident, integer)).padded();

		let binary_operation = value
			.then(
				one_of::<_, _, extra::Err<Rich<'_, char, Span>>>("+*-/!=")
					.repeated()
					.at_least(1)
					.labelled("operator")
					.to_slice(),
			)
			.then(value)
			.labelled("binary_operation")
			.map(|((lhs, op), rhs)| {
				Expression::BinaryOperation(Box::new(lhs), op, Box::new(rhs))
			});

		let let_in = just("let")
			.ignore_then(
				ident
					.then_ignore(just('=').padded())
					.then(expr.clone())
					.padded()
					.then_ignore(just(';'))
					.repeated()
					.collect()
					.padded()
					.labelled("binding"),
			)
			.then_ignore(just("in"))
			.then(expr)
			.labelled("let_in")
			.padded()
			.map(|(bindings, body)| Expression::LetIn {
				bindings,
				body: Box::new(body),
			});

		choice((let_in, binary_operation, value))
	})
}
