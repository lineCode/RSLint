use crate::diagnostic::ParserDiagnostic;
use crate::lexer::token::{BinToken, TokenType};
use crate::parser::cst::expr::*;
use crate::parser::error::ParseDiagnosticType::*;
use crate::parser::Parser;
use crate::span::Span;

impl<'a> Parser<'a> {
    pub fn parse_unary_expr(
        &mut self,
        leading: Option<Span>,
    ) -> Result<Expr, ParserDiagnostic> {
        let leading_whitespace = if leading.is_none() {
            self.whitespace(true)?
        } else {
            leading.unwrap()
        };

        match self.cur_tok.token_type {
            t @ TokenType::Increment | t @ TokenType::Decrement => {
                let start = self.cur_tok.lexeme.start;
                // Advance over the token
                self.advance_lexer(false)?;
                let after = self.whitespace(false)?;
                let object = Box::new(self.parse_unary_expr(None)?);
                let end = object.span().end;

                if !object.is_valid_assign_target(self) {
                    let err = self
                        .error(
                            InvalidTargetExpression,
                            &format!("Invalid left hand side expression for prefix {:?}", t),
                        )
                        .secondary(
                            start..start + 2,
                            &format!("Prefix {:?} operation used here", t),
                        )
                        .primary(
                            object.span().to_owned(),
                            "Not a valid expression for the operator",
                        );
                    self.errors.push(err);
                }

                return Ok(Expr::Update(UpdateExpr {
                    span: self.span(start, end),
                    prefix: true,
                    object,
                    op: t,
                    whitespace: LiteralWhitespace {
                        before: leading_whitespace,
                        after,
                    },
                }));
            }

            t @ TokenType::Delete
            | t @ TokenType::Void
            | t @ TokenType::Typeof
            | t @ TokenType::BinOp(BinToken::Add)
            | t @ TokenType::BinOp(BinToken::Subtract)
            | t @ TokenType::BitwiseNot
            | t @ TokenType::LogicalNot => {
                let start = self.cur_tok.lexeme.start;
                self.advance_lexer(false)?;
                let after = self.whitespace(false)?;
                let object = self.parse_unary_expr(None)?;
                let end = object.span().end;

                if self.state.strict.is_some() && t == TokenType::Delete {
                    if let Expr::Identifier(ref data) = object {
                        let err = self.error(IdentifierDeletion, "`delete` cannot be applied to identifiers in strict mode code")
                            .primary(data.span, "Attempting to delete this identifier is invalid")
                            .help("Help: `delete` is used to delete object properties");

                        self.errors.push(err);
                    }
                }

                return Ok(Expr::Unary(UnaryExpr {
                    span: self.span(start, end),
                    object: Box::new(object),
                    op: t,
                    whitespace: LiteralWhitespace {
                        before: leading_whitespace,
                        after,
                    },
                }));
            }

            _ => {}
        }

        let object = Box::new(self.parse_lhs_expr(Some(leading_whitespace))?);
        let start = object.span().start;
        let mut had_linebreak = self.cur_tok.token_type == TokenType::Linebreak;

        let next: Option<TokenType>;
        if self.cur_tok.token_type != TokenType::Increment
            && self.cur_tok.token_type != TokenType::Decrement
        {
            loop {
                match self.peek_lexer()?.map(|x| x.token_type) {
                    Some(TokenType::Whitespace) => continue,
                    Some(TokenType::Linebreak) => {
                        had_linebreak = true;
                        continue;
                    }
                    t @ _ => {
                        next = t;
                        break;
                    }
                }
            }
            self.lexer.reset();

            if next != Some(TokenType::Increment) && next != Some(TokenType::Decrement) {
                return Ok(*object);
            }
        }

        // A linebreak between an expr and a postfix update is not allowed, therefore we need to return here
        if had_linebreak {
            return Ok(*object);
        }

        let before = self.whitespace(true)?;
        let op_span = self.cur_tok.lexeme.to_owned();
        let op = self.cur_tok.token_type;
        let end = self.cur_tok.lexeme.end;
        self.advance_lexer(false)?;
        let after = self.whitespace(false)?;

        if !object.is_valid_assign_target(self) {
            let err = self
                .error(
                    InvalidTargetExpression,
                    &format!("Invalid left hand side expression for postfix {:?}", op),
                )
                .secondary(op_span, &format!("Postfix {:?} used here", op))
                .primary(
                    object.span().to_owned(),
                    "Not a valid expression for the operator",
                );
            self.errors.push(err);
        }

        Ok(Expr::Update(UpdateExpr {
            span: self.span(start, end),
            prefix: false,
            object,
            op,
            whitespace: LiteralWhitespace {
                before,
                after,
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::expr;
    use crate::lexer::token::TokenType;
    use crate::parser::cst::expr::*;
    use crate::parser::Parser;
    use crate::span;
    use crate::span::Span;

    #[test]
    fn unary_prefix_update() {
        let mut parser = Parser::with_source("--foo \n++5", 0, true).unwrap();
        let first = parser.parse_unary_expr(None);
        let second = parser.parse_unary_expr(None);
        assert_eq!(
            first,
            Ok(Expr::Update(UpdateExpr {
                span: Span::new(0, 5),
                object: Box::new(Expr::Identifier(LiteralExpr {
                    span: Span::new(2, 5),
                    whitespace: LiteralWhitespace {
                        before: Span::new(2, 2),
                        after: Span::new(5, 6),
                    }
                })),
                prefix: true,
                op: TokenType::Decrement,
                whitespace: LiteralWhitespace {
                    before: Span::new(0, 0),
                    after: Span::new(2, 2)
                }
            }))
        );
        assert_eq!(
            second,
            Ok(Expr::Update(UpdateExpr {
                span: Span::new(7, 10),
                object: Box::new(Expr::Number(LiteralExpr {
                    span: Span::new(9, 10),
                    whitespace: LiteralWhitespace {
                        before: Span::new(9, 9),
                        after: Span::new(10, 10),
                    }
                })),
                prefix: true,
                op: TokenType::Increment,
                whitespace: LiteralWhitespace {
                    before: Span::new(6, 7),
                    after: Span::new(9, 9)
                }
            }))
        );
    }

    #[test]
    fn postfix_unary_valid_target() {
        let mut parser = Parser::with_source("mark++", 0, true).unwrap();
        let res = parser.parse_unary_expr(None).unwrap();
        assert_eq!(
            res,
            Expr::Update(UpdateExpr {
                span: Span::new(0, 6),
                object: Box::new(Expr::Identifier(LiteralExpr {
                    span: Span::new(0, 4),
                    whitespace: LiteralWhitespace {
                        before: Span::new(0, 0),
                        after: Span::new(4, 4),
                    }
                })),
                prefix: false,
                op: TokenType::Increment,
                whitespace: LiteralWhitespace {
                    before: Span::new(4, 4),
                    after: Span::new(6, 6),
                }
            })
        );
    }

    #[test]
    fn postfix_unary_with_whitespace() {
        let mut parser = Parser::with_source("\nmk -- \n\n", 0, true).unwrap();
        let res = parser.parse_unary_expr(None).unwrap();
        assert_eq!(
            res,
            Expr::Update(UpdateExpr {
                span: Span::new(1, 6),
                object: Box::new(Expr::Identifier(LiteralExpr {
                    span: Span::new(1, 3),
                    whitespace: LiteralWhitespace {
                        before: Span::new(0, 1),
                        after: Span::new(3, 4)
                    }
                })),
                prefix: false,
                op: TokenType::Decrement,
                whitespace: LiteralWhitespace {
                    before: Span::new(4, 4),
                    after: Span::new(6, 7),
                }
            })
        )
    }

    #[test]
    fn postfix_unary_invalid_target() {
        let mut parser = Parser::with_source("true++", 0, true).unwrap();
        let res = parser.parse_unary_expr(None).unwrap();
        assert_eq!(
            res,
            Expr::Update(UpdateExpr {
                span: Span::new(0, 6),
                object: Box::new(Expr::True(LiteralExpr {
                    span: Span::new(0, 4),
                    whitespace: LiteralWhitespace {
                        before: Span::new(0, 0),
                        after: Span::new(4, 4),
                    }
                })),
                prefix: false,
                op: TokenType::Increment,
                whitespace: LiteralWhitespace {
                    before: Span::new(4, 4),
                    after: Span::new(6, 6),
                }
            })
        );
        assert_eq!(parser.errors.len(), 1);
    }

    #[test]
    fn prefix_update_valid_target() {
        let mut parser = Parser::with_source(" ++ foo ", 0, true).unwrap();
        let res = parser.parse_unary_expr(None).unwrap();
        assert_eq!(
            res,
            Expr::Update(UpdateExpr {
                span: Span::new(1, 7),
                object: Box::new(Expr::Identifier(LiteralExpr {
                    span: Span::new(4, 7),
                    whitespace: LiteralWhitespace {
                        before: Span::new(4, 4),
                        after: Span::new(7, 8)
                    }
                })),
                prefix: true,
                op: TokenType::Increment,
                whitespace: LiteralWhitespace {
                    before: Span::new(0, 1),
                    after: Span::new(3, 4),
                }
            })
        )
    }

    #[test]
    fn prefix_unary() {
        let mut parser = Parser::with_source("delete the_world", 0, true).unwrap();
        let res = parser.parse_unary_expr(None).unwrap();
        assert_eq!(
            res,
            Expr::Unary(UnaryExpr {
                span: Span::new(0, 16),
                object: Box::new(Expr::Identifier(LiteralExpr {
                    span: Span::new(7, 16),
                    whitespace: LiteralWhitespace {
                        before: Span::new(7, 7),
                        after: Span::new(16, 16)
                    }
                })),
                op: TokenType::Delete,
                whitespace: LiteralWhitespace {
                    before: Span::new(0, 0),
                    after: Span::new(6, 7)
                }
            })
        )
    }

    #[test]
    fn grouping() {
        assert_eq!(
            expr!("(/aa/g) "),
            Expr::Grouping(GroupingExpr {
                span: span!("(/aa/g) ", "(/aa/g)"),
                expr: Box::new(Expr::Regex(LiteralExpr {
                    span: span!("(/aa/g) ", "/aa/g"),
                    whitespace: LiteralWhitespace {
                        before: Span::new(1, 1),
                        after: Span::new(6, 6),
                    }
                })),
                opening_paren_whitespace: LiteralWhitespace {
                    before: Span::new(0, 0),
                    after: Span::new(1, 1),
                },
                closing_paren_whitespace: LiteralWhitespace {
                    before: Span::new(6, 6),
                    after: Span::new(7, 8),
                }
            })
        )
    }

    #[test]
    fn nested_grouping() {
        assert_eq!(
            expr!("((foo))"),
            Expr::Grouping(GroupingExpr {
                span: span!("((foo))", "((foo))"),
                expr: Box::new(Expr::Grouping(GroupingExpr {
                    span: span!("((foo))", "(foo)"),
                    expr: Box::new(Expr::Identifier(LiteralExpr {
                        span: span!("((foo))", "foo"),
                        whitespace: LiteralWhitespace {
                            before: Span::new(2, 2),
                            after: Span::new(5, 5),
                        },
                    })),
                    opening_paren_whitespace: LiteralWhitespace {
                        before: Span::new(1, 1),
                        after: Span::new(2, 2),
                    },
                    closing_paren_whitespace: LiteralWhitespace {
                        before: Span::new(5, 5),
                        after: Span::new(6, 6),
                    },
                })),
                opening_paren_whitespace: LiteralWhitespace {
                    before: Span::new(0, 0),
                    after: Span::new(1, 1),
                },
                closing_paren_whitespace: LiteralWhitespace {
                    before: Span::new(6, 6),
                    after: Span::new(7, 7),
                }
            })
        )
    }
}
