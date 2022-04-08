//! # plbot_parser
//! Parser crate for pagelist-bot
//! 

extern crate lalrpop_util;
extern crate unescape;

mod ast;
mod grammar;
mod optim;
mod convert;
mod error;
pub(crate) mod ir;

pub use error::PLBotParserError;

pub type Query = (Vec<ir::Instruction>, ir::RegID);

type PLBotParseResult = Result<Query, PLBotParserError>;

pub fn parse(src: &str) -> PLBotParseResult {
    let ast_res = grammar::ExprParser::new().parse(src);
    let ast = match ast_res {
        Ok(e) => {
            e
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
