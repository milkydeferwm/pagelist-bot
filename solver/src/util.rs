
use crate::error::SolveError;

use plbot_base::bot::APIAssertType;
use plbot_base::ir::RegID;

use std::collections::{HashSet, HashMap};
use mediawiki::title::Title;


use crate::Register;

pub(crate) fn get_set_1<'a>(reg: &'a Register, reg_id: &'a RegID) -> Result<&'a HashSet<Title>, SolveError> {
    let set = reg.get(reg_id);
    if set.is_none() {
        Err(SolveError::UnknownIntermediateValue)
    } else {
        Ok(set.unwrap())
    }
}

pub(crate) fn get_set_2<'a>(reg: &'a Register, reg_id1: &'a RegID, reg_id2: &'a RegID) -> Result<(&'a HashSet<Title>, &'a HashSet<Title>), SolveError> {
    let set1 = reg.get(reg_id1);
    let set2 = reg.get(reg_id2);
    if set1.is_none() || set2.is_none() {
        Err(SolveError::UnknownIntermediateValue)
    } else {
        Ok((set1.unwrap(), set2.unwrap()))
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

pub(crate) fn detect_api_failure(v: &serde_json::Value) -> Result<(), SolveError> {
    if let Some(e) = v["error"].as_object() {
        let ecode = e["code"].as_str().unwrap_or("<unknown>");
        let einfo = e["code"].as_str().unwrap_or("<unknown>");
        return Err(SolveError::from((String::from(ecode), String::from(einfo))));
    }
    Ok(())
}
