pub mod ast;
pub mod error;

pub use ast::*;
pub use error::ParseError;

use crate::{
    lexer::{Token, TokenKind},
    source::SourceSpan,
};

pub fn parse(tokens: Vec<Token>) -> Result<Program, ParseError> {
    Parser::new(tokens).parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
    scope_depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            scope_depth: 0,
        }
    }

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        Ok(Program { statements })
    }

    fn declaration(&mut self) -> Result<Stmt, ParseError> {
        if self.matches(&TokenKind::Import) {
            if self.scope_depth != 0 {
                return Err(self.error_at_previous("imports are only allowed at global scope"));
            }
            return self.import_statement();
        }
        if self.matches(&TokenKind::Namespace) {
            if self.scope_depth != 0 {
                return Err(self.error_at_previous("namespaces are only allowed at global scope"));
            }
            return self.namespace_statement();
        }

        let is_extern = self.matches(&TokenKind::Extern);
        if is_extern && !self.check(&TokenKind::Let) {
            return Err(self.error_at_previous("extern must be followed by let"));
        }
        if self.matches(&TokenKind::Let) {
            return self.let_statement(is_extern, true);
        }
        if is_extern {
            return Err(self.error_at_previous("extern must be followed by let"));
        }

        self.statement()
    }

    fn import_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        let path = match &self.advance().kind {
            TokenKind::String(path) => path.clone(),
            _ => return Err(self.error_at_previous("expected import path string")),
        };
        let end = self
            .consume(&TokenKind::Semicolon, "expected ';' after import")?
            .span;
        Ok(Stmt::Import {
            path,
            span: start.merge(end),
        })
    }

    fn namespace_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        let mut path = vec![self.consume_identifier("expected namespace name")?];
        while self.matches(&TokenKind::Dot) {
            path.push(self.consume_identifier("expected namespace name after '.'")?);
        }

        self.consume(&TokenKind::LeftBrace, "expected '{' before namespace body")?;
        self.scope_depth += 1;
        let body = self.block_body()?;
        self.scope_depth -= 1;
        let end = self.previous().span;

        Ok(Stmt::Namespace {
            path,
            body,
            span: start.merge(end),
        })
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        if self.matches(&TokenKind::Return) {
            return self.return_statement();
        }
        if self.matches(&TokenKind::If) {
            return self.if_statement();
        }
        if self.matches(&TokenKind::While) {
            return self.while_statement();
        }
        if self.matches(&TokenKind::For) {
            return self.for_statement();
        }
        self.expression_statement(true)
    }

    fn let_statement(
        &mut self,
        is_extern: bool,
        require_semicolon: bool,
    ) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        let name = self.consume_identifier("expected variable name after let")?;
        self.consume(&TokenKind::Equal, "expected '=' after variable name")?;
        let value = self.expression()?;
        let end = if require_semicolon {
            self.consume(&TokenKind::Semicolon, "expected ';' after let statement")?
                .span
        } else {
            value.span()
        };
        Ok(Stmt::Let {
            name,
            value,
            is_extern,
            span: start.merge(end),
        })
    }

    fn return_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        if self.matches(&TokenKind::Semicolon) {
            let span = start.merge(self.previous().span);
            return Ok(Stmt::Return { value: None, span });
        }
        let value = self.expression()?;
        let end = self
            .consume(&TokenKind::Semicolon, "expected ';' after return value")?
            .span;
        Ok(Stmt::Return {
            value: Some(value),
            span: start.merge(end),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        let condition = self.expression()?;
        self.consume(&TokenKind::LeftBrace, "expected '{' after if condition")?;
        self.scope_depth += 1;
        let then_branch = self.block_body()?;
        self.scope_depth -= 1;
        let else_branch = if self.matches(&TokenKind::Else) {
            self.consume(&TokenKind::LeftBrace, "expected '{' after else")?;
            self.scope_depth += 1;
            let body = self.block_body()?;
            self.scope_depth -= 1;
            Some(body)
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: start.merge(self.previous().span),
        })
    }

    fn while_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        let condition = self.expression()?;
        self.consume(&TokenKind::LeftBrace, "expected '{' after while condition")?;
        self.scope_depth += 1;
        let body = self.block_body()?;
        self.scope_depth -= 1;
        Ok(Stmt::While {
            condition,
            body,
            span: start.merge(self.previous().span),
        })
    }

    fn for_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous().span;
        let initializer = if self.matches(&TokenKind::Semicolon) {
            None
        } else if self.matches(&TokenKind::Let) {
            Some(Box::new(self.let_statement(false, true)?))
        } else {
            Some(Box::new(self.expression_statement(true)?))
        };

        let condition = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(&TokenKind::Semicolon, "expected ';' after for condition")?;

        let increment = if self.check(&TokenKind::LeftBrace) {
            None
        } else {
            Some(self.expression()?)
        };
        self.consume(&TokenKind::LeftBrace, "expected '{' before for body")?;
        self.scope_depth += 1;
        let body = self.block_body()?;
        self.scope_depth -= 1;

        Ok(Stmt::For {
            initializer,
            condition,
            increment,
            body,
            span: start.merge(self.previous().span),
        })
    }

    fn expression_statement(&mut self, require_semicolon: bool) -> Result<Stmt, ParseError> {
        let expr = self.expression()?;
        let end = if require_semicolon {
            self.consume(&TokenKind::Semicolon, "expected ';' after expression")?
                .span
        } else {
            expr.span()
        };
        Ok(Stmt::Expr {
            span: expr.span().merge(end),
            expr,
        })
    }

    fn block_body(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        self.consume(&TokenKind::RightBrace, "expected '}' after block")?;
        Ok(statements)
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, ParseError> {
        let expr = self.or()?;
        if self.matches(&TokenKind::Equal) {
            let equals = self.previous().span;
            let value = self.assignment()?;
            match expr {
                Expr::Variable { .. } | Expr::Member { .. } | Expr::Index { .. } => {
                    let span = expr.span().merge(value.span());
                    Ok(Expr::Assignment {
                        target: Box::new(expr),
                        value: Box::new(value),
                        span,
                    })
                }
                _ => Err(ParseError::new("invalid assignment target", equals)),
            }
        } else {
            Ok(expr)
        }
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.and()?;
        while self.matches(&TokenKind::PipePipe) {
            let right = self.and()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.equality()?;
        while self.matches(&TokenKind::AmpAmp) {
            let right = self.equality()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.comparison()?;
        while self.matches_any(&[TokenKind::EqualEqual, TokenKind::BangEqual]) {
            let op = if self.previous_is(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else {
                BinaryOp::NotEqual
            };
            let right = self.comparison()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.term()?;
        while self.matches_any(&[
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
        ]) {
            let op = match &self.previous().kind {
                TokenKind::Less => BinaryOp::Less,
                TokenKind::LessEqual => BinaryOp::LessEqual,
                TokenKind::Greater => BinaryOp::Greater,
                TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
                _ => unreachable!(),
            };
            let right = self.term()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.factor()?;
        while self.matches_any(&[TokenKind::Plus, TokenKind::Minus]) {
            let op = if self.previous_is(&TokenKind::Plus) {
                BinaryOp::Add
            } else {
                BinaryOp::Subtract
            };
            let right = self.factor()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.unary()?;
        while self.matches_any(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let op = match &self.previous().kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                TokenKind::Percent => BinaryOp::Modulo,
                _ => unreachable!(),
            };
            let right = self.unary()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.matches_any(&[TokenKind::Bang, TokenKind::Minus]) {
            let op = if self.previous_is(&TokenKind::Bang) {
                UnaryOp::Not
            } else {
                UnaryOp::Negate
            };
            let start = self.previous().span;
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                span: start.merge(expr.span()),
                op,
                expr: Box::new(expr),
            });
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.primary()?;
        loop {
            if self.matches(&TokenKind::LeftParen) {
                let start = expr.span();
                let args = self.arguments(TokenKind::RightParen)?;
                let end = self.previous().span;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span: start.merge(end),
                };
            } else if self.matches(&TokenKind::Dot) {
                let start = expr.span();
                let field = self.consume_identifier("expected field name after '.'")?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    field,
                    span: start.merge(self.previous().span),
                };
            } else if self.matches(&TokenKind::Arrow) {
                let start = expr.span();
                let method = self.consume_identifier("expected method name after '->'")?;
                self.consume(&TokenKind::LeftParen, "expected '(' after method name")?;
                let args = self.arguments(TokenKind::RightParen)?;
                expr = Expr::MethodCall {
                    receiver: Box::new(expr),
                    method,
                    args,
                    span: start.merge(self.previous().span),
                };
            } else if self.matches(&TokenKind::LeftBracket) {
                expr = self.index_or_slice(expr)?;
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn index_or_slice(&mut self, object: Expr) -> Result<Expr, ParseError> {
        let start_span = object.span();
        if self.matches(&TokenKind::Colon) {
            let end = if self.check(&TokenKind::RightBracket) {
                None
            } else {
                Some(Box::new(self.expression()?))
            };
            self.consume(&TokenKind::RightBracket, "expected ']' after slice")?;
            return Ok(Expr::Slice {
                object: Box::new(object),
                start: None,
                end,
                span: start_span.merge(self.previous().span),
            });
        }

        let first = self.expression()?;
        if self.matches(&TokenKind::Colon) {
            let end = if self.check(&TokenKind::RightBracket) {
                None
            } else {
                Some(Box::new(self.expression()?))
            };
            self.consume(&TokenKind::RightBracket, "expected ']' after slice")?;
            Ok(Expr::Slice {
                object: Box::new(object),
                start: Some(Box::new(first)),
                end,
                span: start_span.merge(self.previous().span),
            })
        } else {
            self.consume(&TokenKind::RightBracket, "expected ']' after index")?;
            Ok(Expr::Index {
                object: Box::new(object),
                index: Box::new(first),
                span: start_span.merge(self.previous().span),
            })
        }
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Null => Ok(Expr::Null(token.span)),
            TokenKind::True => Ok(Expr::Bool(true, token.span)),
            TokenKind::False => Ok(Expr::Bool(false, token.span)),
            TokenKind::Int(value) => Ok(Expr::Int(value, token.span)),
            TokenKind::Float(value) => Ok(Expr::Float(value, token.span)),
            TokenKind::String(value) => Ok(Expr::String(value, token.span)),
            TokenKind::Identifier(name) => Ok(Expr::Variable {
                name,
                span: token.span,
            }),
            TokenKind::Fn => self.function_expression(token.span),
            TokenKind::Struct => self.struct_expression(token.span),
            TokenKind::LeftBracket => self.array_expression(token.span),
            TokenKind::New => self.new_array_expression(token.span),
            TokenKind::LeftParen => {
                let expr = self.expression()?;
                self.consume(&TokenKind::RightParen, "expected ')' after expression")?;
                Ok(expr)
            }
            _ => Err(ParseError::new("expected expression", token.span)),
        }
    }

    fn function_expression(&mut self, start: SourceSpan) -> Result<Expr, ParseError> {
        self.consume(&TokenKind::LeftParen, "expected '(' after fn")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let span = self.peek().span;
                let name = self.consume_identifier("expected parameter name")?;
                let is_rest = self.matches(&TokenKind::DotDotDot);
                if is_rest && !self.check(&TokenKind::RightParen) {
                    return Err(ParseError::new(
                        "rest parameter must be the final parameter",
                        span,
                    ));
                }
                params.push(Param {
                    name,
                    is_rest,
                    span,
                });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RightParen, "expected ')' after parameters")?;
        self.consume(&TokenKind::LeftBrace, "expected '{' before function body")?;
        self.scope_depth += 1;
        let body = self.block_body()?;
        self.scope_depth -= 1;
        Ok(Expr::Function {
            params,
            body,
            span: start.merge(self.previous().span),
        })
    }

    fn struct_expression(&mut self, start: SourceSpan) -> Result<Expr, ParseError> {
        self.consume(&TokenKind::LeftBrace, "expected '{' after struct")?;
        let mut fields = Vec::new();
        if !self.check(&TokenKind::RightBrace) {
            loop {
                self.consume(&TokenKind::Dot, "expected '.' before struct field")?;
                let name = self.consume_identifier("expected struct field name")?;
                self.consume(&TokenKind::Colon, "expected ':' after struct field name")?;
                let value = self.expression()?;
                fields.push((name, value));
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
                if self.check(&TokenKind::RightBrace) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RightBrace, "expected '}' after struct")?;
        Ok(Expr::Struct {
            fields,
            span: start.merge(self.previous().span),
        })
    }

    fn array_expression(&mut self, start: SourceSpan) -> Result<Expr, ParseError> {
        let mut values = Vec::new();
        if !self.check(&TokenKind::RightBracket) {
            loop {
                values.push(self.expression()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
                if self.check(&TokenKind::RightBracket) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RightBracket, "expected ']' after array")?;
        Ok(Expr::Array {
            values,
            span: start.merge(self.previous().span),
        })
    }

    fn new_array_expression(&mut self, start: SourceSpan) -> Result<Expr, ParseError> {
        self.consume(&TokenKind::LeftBracket, "expected '[' after new")?;
        let size = self.expression()?;
        self.consume(&TokenKind::RightBracket, "expected ']' after array size")?;
        Ok(Expr::NewArray {
            span: start.merge(self.previous().span),
            size: Box::new(size),
        })
    }

    fn arguments(&mut self, terminator: TokenKind) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if !self.check(&terminator) {
            loop {
                args.push(self.expression()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(&terminator, "expected ')' after arguments")?;
        Ok(args)
    }

    fn consume_identifier(&mut self, message: &str) -> Result<String, ParseError> {
        match self.advance().kind.clone() {
            TokenKind::Identifier(name) => Ok(name),
            _ => Err(self.error_at_previous(message)),
        }
    }

    fn consume(&mut self, kind: &TokenKind, message: &str) -> Result<&Token, ParseError> {
        if self.check(kind) {
            return Ok(self.advance());
        }
        Err(ParseError::new(message, self.peek().span))
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn matches_any(&mut self, kinds: &[TokenKind]) -> bool {
        if kinds.iter().any(|kind| self.check(kind)) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        same_variant(&self.peek().kind, kind)
    }

    fn previous_is(&self, kind: &TokenKind) -> bool {
        same_variant(&self.previous().kind, kind)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn error_at_previous(&self, message: &str) -> ParseError {
        ParseError::new(message, self.previous().span)
    }
}

fn same_variant(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_source(source: &str) -> Result<Program, ParseError> {
        parse(lex(source).unwrap())
    }

    #[test]
    fn parses_required_expressions() {
        parse_source(
            r#"
            let f = fn(a, rest...) { return a; };
            let obj = struct { .name: "Colby" };
            let arr = [1, 2, 3];
            let sized = new [10];
            let remainder = 10 % 3;
            arr[0] = 1;
            arr[1:3];
            object->method(1, 2);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn rejects_invalid_rest_parameter() {
        assert!(parse_source("let f = fn(args..., other) {};").is_err());
    }

    #[test]
    fn rejects_import_inside_namespace() {
        assert!(parse_source(r#"namespace A { import "x.crs"; }"#).is_err());
    }
}
