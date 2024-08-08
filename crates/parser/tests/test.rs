use ariadne::{sources, Color, Label, Report, ReportKind};
use chumsky::Parser;
use smolix_parser::{expression, Expression};

fn parse(filename: String, input: &str) -> Option<Expression<'_>> {
	let (expression, errors) = expression().parse(input).into_output_errors();

	errors
		.into_iter()
		.map(|e| e.map_token(|c| c.to_string()))
		.for_each(|e| {
			Report::build(ReportKind::Error, filename.clone(), e.span().start)
				.with_message(e.to_string())
				.with_label(
					Label::new((filename.clone(), e.span().into_range()))
						.with_message(e.reason())
						.with_color(Color::Red),
				)
				.with_labels(e.contexts().map(|(label, span)| {
					Label::new((filename.clone(), span.into_range()))
						.with_message(format!("while parsing this {}", label))
						.with_color(Color::Yellow)
				}))
				.finish()
				.print(sources([(filename.clone(), input)]))
				.unwrap()
		});

	expression
}

#[test]
fn test() {
	let input = r#"
        let
            x = 2;
            y = 3;
            z = x + y;
        in z
    "#;

	assert!(parse("stdin".to_string(), input).is_some());
}
