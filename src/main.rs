use warp::Filter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;
use std::process::Command;
use tokio::sync::Mutex;


// struct expected from first POST request
#[derive(Deserialize, Serialize, Debug, Clone)]
struct TestData {
    device: String,
    check: bool,
    int_data: i32,
    float_data: f64,
}

#[derive(Deserialize,Serialize)]
struct TestResult {
    pass: bool,
}


// handle POST req
fn handle_post(new_data: TestData, data_store: Arc<Mutex<Vec<TestData>>>) -> impl warp::Reply {
    println!("Received JSON data: {:?}", new_data);

    let script = format!("/home/root/OpenAMP-Example/fw_cortex_m4.sh").to_owned();
    let script_path = Path::new(&script);
    let output = Command::new("sh")
        .arg("-c")
        .arg(script_path)
        .output()
        .expect("Failed to run script!");
    println!("{:?}",output.stdout);

    
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
    
    warp::reply::json(&serde_json::json!({"status": 0}))
}

#[tokio::main]
async fn main() {
    // Create a shared data store
    let data_store = Arc::new(Mutex::new(Vec::new()));

    // CORS configuration
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["Content-Type"])
        .allow_methods(vec!["POST", "OPTIONS"]);

    // Clone the Arc for the filter
    let data_store_filter = warp::any()
        .map(move || data_store.clone());

    let dev_selecet_route = warp::post()
        .and(warp::body::json())
        .and(data_store_filter)
        .map(handle_post);
    
    // OPTIONS route for CORS preflight requests
    let options_route = warp::options()
        .map(|| warp::reply());

    // Combine routes
    let routes = dev_selecet_route
        .or(options_route)
        .with(cors);

    println!("Server starting on http://localhost:8080");
    warp::serve(routes)
        .run(([172,20,10,7], 8080))
        .await;
}
