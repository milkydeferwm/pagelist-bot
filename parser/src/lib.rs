//! # plbot_parser
//! Parser crate for pagelist-bot
//! 

extern crate plbot_base;
extern crate lalrpop_util;
extern crate unescape;

mod ast;
mod grammar;
mod optim;
mod convert;
mod error;

pub use error::PLBotParserError;

type PLBotParseResult = Result<plbot_base::Query, PLBotParserError>;

pub fn parse(src: &str) -> PLBotParseResult {
    let ast_res = grammar::ExprParser::new().parse(src);
    let ast;
    match ast_res {
        Ok(e) => {
            ast = e;
        },
        Err(_) => {
            return Err(PLBotParserError::Parse);
        },
    };
    let (mut ir_ls, ir_fin) = convert::to_ir(&ast)?;
    optim::remove_redundent_talk(&mut ir_ls);
    optim::remove_empty_ns(&mut ir_ls);

    optim::remove_nop(&mut ir_ls);
    Ok((ir_ls, ir_fin))
}
