use warp::Filter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

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


// Function to handle the POST request
fn handle_post(new_data: TestData, data_store: Arc<Mutex<Vec<TestData>>>) -> impl warp::Reply {
    println!("Received JSON data: {:?}", new_data);
    
    // Clone new_data before moving it into the async block
    let new_data_clone = new_data.clone();
    
    // Spawn a new task to process the data asynchronously
    tokio::spawn(async move {
        let mut store = data_store.lock().await;
        store.push(new_data_clone);
        println!("Updated data store. Current count: {}", store.len());
    });
    
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
    let data_store_filter = warp::any().map(move || data_store.clone());

    let post_route = warp::post()
        .and(warp::body::json())
        .and(data_store_filter)
        .map(handle_post);

    // OPTIONS route for CORS preflight requests
    let options_route = warp::options()
        .map(|| warp::reply());

    // Combine routes
    let routes = post_route
        .or(options_route)
        .with(cors);

    println!("Server starting on http://localhost:8080");
    warp::serve(routes)
        .run(([192,168,56,1], 8080))
        .await;
}
