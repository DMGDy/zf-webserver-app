use warp::{Filter,Rejection,Reply};
use colored::*;
use tokio::sync::Mutex;
use std::path::Path;
use std::{
    sync::Arc,
    thread,
    time::Duration,
};

use crate::test::State;
mod test;

type ShatedState<T> = Arc<Mutex<Vec<T>>>;

async
fn handle_get_results(data_store: ShatedState<test::TestData>)
-> Result<impl Reply,Rejection> {
    // get Test Data from previous request
    // TODO: make this not unsafe and not be lazy with unwraps
    let data= tokio::spawn(async move {

        let store = data_store.lock().await;
        store.last().unwrap().clone()
    }).await.unwrap();
    
    let mut test_result;

    loop {
        // loop until test is done witha  pass or fail
        test_result = test::get_results();
        if matches!(test_result,State::Pass|State::Fail) {
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }

    /* read trace from csv before deloading firmware
    * located at 
    * `/sys/kernel/debug/remoteproc/remoteproc0/trace0`
    */

    test::trace_to_csv(data.abbrv_device());
           
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

    Ok(warp::reply::json(&test_result))
}

// handle POST req
fn handle_post(new_data: test::TestData, 
    data_store: ShatedState<test::TestData>) -> impl Reply {
    let response = test::begin_test(&new_data);
   
    let new_data_clone = new_data.clone();
    
    // Spawn a new task to process the data asynchronously
    let store_copy = data_store.clone();
    tokio::spawn(async move {
        let mut store = store_copy.lock().await;
        store.push(new_data_clone);
        println!("Updated data store. Current count: {}", store.len());
    });

   // using code() method to ensure enum value aligns with web applications
   warp::reply::json(&response)
}


/* main function: configures server with warp filters for incoming
 *  requests.
 */
#[tokio::main]
async fn main() {
    let data_store: ShatedState<test::TestData> = Arc::new(Mutex::new(Vec::new()));

    // configure CORs
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type"])
        .allow_methods(vec!["GET", "POST", "OPTIONS"]);

    // so so test data can be passed around
    let data_store_filter = warp::any()
        .map(move || data_store.clone());

    let get_data = warp::get()
        .and(warp::path::param())
        .and(warp::path::end())
        .map(|dev: String| {
            let csv = format!("{}-test.csv",dev.clone());
            println!("Serving {csv} for download to client");
            let data_path = Path::new("data/");
            warp::fs::file(data_path.join(csv))
        })
        .map(|_| warp::reply())
        .map(|response| {
            warp::reply::with_header(
                response,
                "Content-Type",
                "text/csv"
            )
        });

    let is_up_route = warp::get()
        .and(warp::path("up"))
        .map(|| warp::reply::json(&State::Online));

    let dev_select_route = warp::post()
        .and(warp::body::json())
        .and(data_store_filter.clone())
        .map(handle_post);
    
    let result_route = warp::get()
        .and(warp::path("result"))
        .and(data_store_filter)
        .and_then(handle_get_results);

    let options_route = warp::options()
        .map(|| warp::reply());

    // routes
    let routes = options_route
        .or(is_up_route)
        .or(dev_select_route)
        .or(result_route)
        .or(get_data)
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
