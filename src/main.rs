use warp::Filter;
use colored::*;
use tokio::sync::Mutex;
use std::{
    sync::Arc,
    thread,
    time::Duration,
};

use crate::test::State;
mod test;


fn handle_get_results(data_store: Arc<Mutex<Vec<test::TestData>>>) 
-> impl warp::Reply {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // get Test Data from previous request
    // TODO: make this not unsafe and not be lazy with unwraps
    let dataresult = tokio::spawn(async move {

        let mut store = data_store.lock().await;
        store.pop().unwrap()
    });
    
    let data = rt.block_on(dataresult).unwrap();
    let mut test_state;

    loop {
        // loop until test is done witha  pass or fail
        test_state = test::get_results();
        if matches!(test_state,State::Pass|State::Fail) {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }

   match test::m4_firmware(data.abbrv_device(),test::FimwareOption::STOP) {
       Ok(output) => {
           println!("Firmware for {} has been deloaded: {}"
               ,data.abbrv_device(), String::from_utf8_lossy(&output.stdout));
           println!("---------------------------------------------------")
       },
       Err(e) => {
           println!("{}: {}","Error deloading firmware!".red().bold(),e);
           println!("{}","Stopping the Server...".italic().red());
           std::process::exit(-1)
       }
   }
   
    warp::reply::json(&(test_state.code()))
}

// handle POST req
fn handle_post(new_data: test::TestData, data_store: Arc<Mutex<Vec<test::TestData>>>) -> impl warp::Reply {
    let response = test::begin_test(&new_data);
   
    let new_data_clone = new_data.clone();
    
    // Spawn a new task to process the data asynchronously
    tokio::spawn(async move {
        let mut store = data_store.lock().await;
        store.push(new_data_clone);
        println!("Updated data store. Current count: {}", store.len());
    });

   // using code() method to ensure enum value aligns with web applications
   warp::reply::json(&(response.code()))
}


/* main function: configures server with warp filters for incoming
 *  requests.
 */
#[tokio::main]
async fn main() {
    let data_store = Arc::new(Mutex::new(Vec::new()));

    // configure CORs
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type"])
        .allow_methods(vec!["GET", "POST", "OPTIONS"]);

    // so so test data can be passed around
    let data_store_filter = warp::any()
        .map(move || data_store.clone());

    let is_up_route = warp::get()
        .and(warp::path("up"))
        .map(|| warp::reply::json(&State::Online));

    let dev_selecet_route = warp::post()
        .and(warp::body::json())
        .and(data_store_filter.clone())
        .map(handle_post);
    
    
    let result_route = warp::get()
        .and(warp::path("result"))
        .and(data_store_filter)
        .map(handle_get_results.clone());

    let options_route = warp::options()
        .map(|| warp::reply());

    // routes
    let routes = options_route
        .or(is_up_route)
        .or(dev_selecet_route)
        .or(result_route)
        .with(cors);

    println!("-----{}-----",
        "Rust Server for Web Assembly Application".bold().underline());

    println!("---------------------------------------------------");
    println!("{}","Server Listening http://172.20.10.7:8080...".italic().bold());
    println!("---------------------------------------------------");
    warp::serve(routes)
        .run(([172,20,10,7], 8080))
        .await
}
