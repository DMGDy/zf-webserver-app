use warp::Filter;
use colored::*;

use crate::test::State;
mod test;

fn handle_get() -> impl warp::Reply {

    warp::reply::json(&serde_json::json!({"status":"0"}))
}


// handle POST req, first request received when "Start Test" is clicked
fn handle_post(new_data: test::TestData) -> impl warp::Reply {
    let response = test::begin_test(&new_data);
   
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
   // using code() method to ensure enum value aligns with web applications
   warp::reply::json(&(response.code()))
}


/* main function: configures server with warp filters for incoming
 *  requests.
 */
#[tokio::main]
async fn main() {

    // configure CORs
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type"])
        .allow_methods(vec!["POST", "OPTIONS", "GET"]);

    let is_up_route = warp::get()
        .and(warp::path("up"))
        .map(|| warp::reply::json(&State::Online));

    let dev_selecet_route = warp::post()
        .and(warp::body::json())
        .map(handle_post);
    
    
    let test_route = warp::get()
        .map(handle_get);

    let options_route = warp::options()
        .map(|| warp::reply());

    // routes
    let routes = options_route
        .or(is_up_route)
        .or(dev_selecet_route)
        .or(test_route)
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
