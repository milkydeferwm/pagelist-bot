
use super::error::SolveError;

use crate::types::APIAssertType;
use crate::parser::ir::RegID;

use std::collections::{HashSet, HashMap};
use mediawiki::title::Title;

use super::Register;

pub(crate) fn get_set_1<'a>(reg: &'a Register, reg_id: &'a RegID) -> Result<&'a HashSet<Title>, SolveError> {
    let set = reg.get(reg_id);
    if let Some(s) = set {
        Ok(s)
    } else {
        Err(SolveError::UnknownIntermediateValue)
    }
}

pub(crate) fn get_set_2<'a>(reg: &'a Register, reg_id1: &'a RegID, reg_id2: &'a RegID) -> Result<(&'a HashSet<Title>, &'a HashSet<Title>), SolveError> {
    let set1 = reg.get(reg_id1);
    let set2 = reg.get(reg_id2);
    if let (Some(s1), Some(s2)) = (set1, set2) {
        Ok((s1, s2))
    } else {
        Err(SolveError::UnknownIntermediateValue)
    }
}

pub(crate) fn insert_assert_param(params: &mut HashMap<String, String>, assert: Option<APIAssertType>) {
    if let Some(a) = assert {
        params.insert("assert".to_string(), a.to_string());
    };
}

pub(crate) fn concat_params<T>(v: &HashSet<T>) -> String 
where
    T: ToString,
{
    v.iter().map(|f| T::to_string(f)).collect::<Vec<String>>().join("|")
}
