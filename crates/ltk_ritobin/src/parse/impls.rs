use crate::parse::{cst::Kind as TreeKind, error::ErrorKind, parser::Parser, tokenizer::TokenKind};

use TokenKind::*;

pub fn file(p: &mut Parser) {
    let m = p.open();
    while !p.eof() {
        if p.at(Comment) {
            p.scope(TreeKind::Comment, |p| p.advance());
        }
        stmt_or_list_item(p)
    }
    p.close(m, TreeKind::File);
}

pub fn stmt_or_list_item(p: &mut Parser) {
    let m = p.open();

    // obvious list item literals
    if p.eat_any(&[TokenKind::HexLit, TokenKind::Number]) {
        p.advance(); // list item
        while p.eat(Newline) {}
        p.close(m, TreeKind::ListItem);
        return;
    }

    p.scope(TreeKind::EntryKey, |p| {
        p.expect_any(&[TokenKind::Name, TokenKind::String])
    });
    match p.nth(0) {
        TokenKind::Colon => {
            p.advance();
            type_expr(p);
            p.expect(TokenKind::Eq);
        }
        TokenKind::Eq => {
            p.advance();
        }
        _ => {
            p.advance(); // list item
            while p.eat(Newline) {}
            p.close(m, TreeKind::ListItem);
            return;
        }
    }

    if !entry_value(p) {
        p.close(m, TreeKind::Entry);
        return;
    }
    p.scope(TreeKind::EntryTerminator, |p| {
        let mut one = false;
        if p.eof() {
            return;
        }
        while p.eat_any(&[TokenKind::SemiColon, TokenKind::Newline]) {
            one = true;
        }

        if !one {
            // if something was between us and our statement terminator,
            // we eat it all and then try again
            p.scope(TreeKind::ErrorTree, |p| {
                while !matches!(
                    p.nth(0),
                    TokenKind::SemiColon | TokenKind::Newline | TokenKind::Eof
                ) {
                    p.advance();
                }
                p.report(ErrorKind::UnexpectedTree);
            });
            while p.eat_any(&[TokenKind::SemiColon, TokenKind::Newline]) {}
        }
    });
    p.close(m, TreeKind::Entry);
}

pub fn entry_value(p: &mut Parser) -> bool {
    p.scope(TreeKind::EntryValue, |p| {
        match p.nth(0) {
            TokenKind::String => {
                p.advance();
            }
            TokenKind::UnterminatedString => {
                p.advance_with_error(ErrorKind::UnterminatedString, None);
            }
            Number | Minus | HexLit | True | False => {
                let m = p.open();
                p.advance();
                p.close(m, TreeKind::Literal);
            }
            TokenKind::Name => {
                p.scope(TreeKind::Class, |p| {
                    p.advance();
                    block(p);
                });
            }
            TokenKind::LCurly => {
                block(p);
            }
            TokenKind::Newline => {
                p.advance_with_error(ErrorKind::Unexpected { token: Newline }, None);
                while p.eat(Newline) {}
                return false;
            }
            token @ TokenKind::Eof => p.report(ErrorKind::Unexpected { token }),
            token => p.advance_with_error(ErrorKind::Unexpected { token }, None),
        }
        true
    })
    .0
}

pub fn type_expr(p: &mut Parser) {
    p.scope(TreeKind::TypeExpr, |p| {
        p.expect(TokenKind::Name);
        if p.eat(TokenKind::LBrack) {
            p.scope(TreeKind::TypeArgList, |p| {
                while !p.at(TokenKind::RBrack) && !p.eof() {
                    if p.at(TokenKind::Name) {
                        expr_type_arg(p);
                    } else {
                        break;
                    }
                }
            });
            p.expect(TokenKind::RBrack);
        }
    });
}

pub fn expr_type_arg(p: &mut Parser) {
    assert!(p.at(Name));
    let m = p.open();

    p.expect(Name);
    p.close(m, TreeKind::TypeArg);

    if !p.at(RBrack) {
        p.expect(Comma);
    }
}

pub fn block(p: &mut Parser) {
    assert!(p.at(LCurly));
    let m = p.open();
    p.expect(LCurly);
    while !p.at(RCurly) && !p.eof() {
        stmt_or_list_item(p);
    }
    p.expect(RCurly);

    p.close(m, TreeKind::Block);
}
