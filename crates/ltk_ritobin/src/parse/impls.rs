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
    match (p.nth(0), p.nth(1), p.nth(2)) {
        (Name | String | HexLit, Colon | Eq, _) => {
            stmt(p);
        }
        (Name | HexLit, LCurly, _) => {
            let m = p.open();
            p.advance();
            block(p);
            p.close(m, TreeKind::Class);
        }
        (LCurly, _, _) => {
            let m = p.open();
            block(p);
            p.close(m, TreeKind::ListItem);
            p.eat(Comma);
        }
        (Name | HexLit | String | Number | True | False, _, _) => {
            let m = p.open();
            p.scope(TreeKind::Literal, |p| p.advance());
            p.close(m, TreeKind::ListItem);
            p.eat(Comma);
        }
        _ => stmt(p),
    }

    while p.eat(Newline) {}
}

pub fn stmt(p: &mut Parser) {
    let m = p.open();

    p.scope(TreeKind::EntryKey, |p| {
        p.expect_any(&[Name, String, HexLit])
    });
    if p.eat_any(&[Colon, Eq, Newline]) == Some(Colon) {
        type_expr(p);
        p.expect(TokenKind::Eq);
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
        while p
            .eat_any(&[TokenKind::SemiColon, TokenKind::Newline])
            .is_some()
        {
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
            while p
                .eat_any(&[TokenKind::SemiColon, TokenKind::Newline])
                .is_some()
            {}
        }
    });
    p.close(m, TreeKind::Entry);
}

pub fn entry_value(p: &mut Parser) -> bool {
    p.scope(TreeKind::EntryValue, |p| {
        match (p.nth(0), p.nth(1)) {
            (Name, _) | (HexLit, LCurly) => {
                p.scope(TreeKind::Class, |p| {
                    p.advance();
                    if p.at(LCurly) {
                        block(p);
                    }
                });
            }
            (UnterminatedString, _) => {
                p.advance_with_error(ErrorKind::UnterminatedString, None);
            }
            (String | Number | HexLit | True | False, _) => {
                p.scope(TreeKind::Literal, |p| p.advance());
            }
            (LCurly, _) => {
                block(p);
            }
            (Newline, _) => {
                p.advance_with_error(ErrorKind::Unexpected { token: Newline }, None);
                while p.eat(Newline) {}
                return false;
            }
            (token @ TokenKind::Eof, _) => p.report(ErrorKind::Unexpected { token }),
            (token, _) => p.advance_with_error(ErrorKind::Unexpected { token }, None),
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
