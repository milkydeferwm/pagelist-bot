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

type PLBotParseResult = Result<plbot_base::Query, Box<dyn std::error::Error>>;

pub fn parse(src: &'static str) -> PLBotParseResult {
    let ast = grammar::ExprParser::new().parse(src)?;
    let (mut ir_ls, ir_fin) = convert::to_ir(&ast)?;
    optim::remove_redundent_talk(&mut ir_ls);
    optim::remove_empty_ns(&mut ir_ls);

    optim::remove_nop(&mut ir_ls);
    Ok((ir_ls, ir_fin))
}
