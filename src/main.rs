mod test;

use warp::Filter;
use std::sync::Arc;
use colored::*;
use tokio::sync::Mutex;


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

    /* TODO: implement on server to comprehend a struct as response
    let result = TestResult {
        pass: true
    };

    //warp::reply::json(&serde_json::json!(result));
    */
    
    /* deloading m4 firmware for now */
   match test::m4_firmware(new_data.abbrv_device(),test::FimwareOption::STOP) {
       Ok(output) => {
           println!("Firmware for {} has been deloaded: {}"
               ,new_data.abbrv_device(), String::from_utf8_lossy(&output.stdout));
           println!("---------------------------------------------------")
       },
       Err(e) => {
           println!("{}: {}","Error deloading firmware!".red().bold(),e);
           println!("{}","Stopping the Server...".italic().red());
           std::process::exit(-1)
       }
   }
   warp::reply::json(&(response.code()))


}

/* main function: configures server with warp filters for incoming
 *  requests.
 *
 */
#[tokio::main]
async fn main() {
    let data_store = Arc::new(Mutex::new(Vec::new()));

    // configure CORs
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type"])
        .allow_methods(vec!["POST", "OPTIONS"]);

    let data_store_filter = warp::any()
        .map(move || data_store.clone());

    let dev_selecet_route = warp::post()
        .and(warp::body::json())
        .and(data_store_filter)
        .map(handle_post);
    
    
    let options_route = warp::options()
        .map(|| warp::reply());

    // routes
    let routes = dev_selecet_route
        .or(options_route)
        .with(cors);

    println!("------{}-----",
        "Rust Server for Web Assembly Application".bold().underline());

    println!("---------------------------------------------------");
    println!("{}","Server Listening http://172.20.10.7:8080...".italic());
    println!("---------------------------------------------------");
    warp::serve(routes)
        .run(([172,20,10,7], 8080))
        .await;
}
