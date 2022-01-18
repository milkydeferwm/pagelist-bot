use mediawiki::api::Api;
use plbot_parser;
use plbot_solver;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    // test
    println!("Here comes the test");
    let mut api: Api;
    let api_result = Api::new("https://zh.wikipedia.org/w/api.php").await;
    if api_result.is_err() {
        println!("Cannot access MediaWiki API of the target website. Quitting.");
        return;
    } else {
        println!("Api fetch successful.");
        api = api_result.unwrap();
    }
    api.set_maxlag(Some(5));
    let query;
    // let query_result = plbot_parser::parse("toggle( incat( page(\"Category:电子游戏条目\") ).ns(0) ) & incat( page(\"Category:典范条目\") ).ns(0)");
    let query_result = plbot_parser::parse("( toggle( incat( page(\"Category:电子游戏条目\") ) ) & incat( page(\"Category:典范条目\") ) ).ns(0)");
    if query_result.is_err() {
        println!("Parse fails.");
        return;
    } else {
        println!("Query parse successful.");
        query = query_result.unwrap();
    }
    let solve_result = plbot_solver::solve_api(&query, &api, None).await;
    if solve_result.is_err() {
        println!("Solve fails. {}", solve_result.unwrap_err());
        return;
    } else {
        println!("Solve success. {} results.", solve_result.unwrap().len());
    }
}
