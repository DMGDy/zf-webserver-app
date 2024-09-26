use warp::Filter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;
use std::process::Command;
use std::fs;
use std::fs::{File,OpenOptions};
use std::error::Error;
use std::io::{BufReader,BufWriter,Write,Read};
use std::os::unix::fs::OpenOptionsExt;
use std::str;
use std::{thread,time};
use libc;
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

impl TestData {
    pub fn abbrv_device(&self) -> &str {
        match self.device.as_str() {
            "Brake Signal Transmitter" => "BST",
            "Continuous Wear Sensor" => "CWS",
            "Pressure Sensor" => "PrS",
            "Electronic Stability Control Module" => "ESCM",
            _ => "ERR",
        }
    }
}

fn ipc_comm() -> Result<(), Box<dyn Error>>{
    let mut msgbuffer = Vec::new();

    let dev_rpmsg = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open("/dev/ttyRPMSG0")?;
    let mut rpmsg_reader = BufReader::new(&dev_rpmsg);
    let mut rpmsg_writer = BufWriter::new(&dev_rpmsg);

    match rpmsg_writer.write(b"test") {
        Ok(_) => {
            let _ = time::Duration::from_millis(10);
            loop {
                match rpmsg_reader.read_to_end(&mut msgbuffer) {
                    Ok(_) => {
                        match str::from_utf8(&msgbuffer) {
                            Ok(s) => {
                                println!("{}",s);
                                break
                            }
                            Err(e) => println!("Error reading from buffer!: {}",e),
                        }
                    }
                    Err(e) => println!("Error reading file!: {}",e),
                }
            };
        },
        Err(e) => {
            println!("Error writing!: {}",e)
        },
    };

    Ok(())
}


// handle POST req
fn handle_post(new_data: TestData, data_store: Arc<Mutex<Vec<TestData>>>) -> impl warp::Reply {
    println!("Test Info:\nDevice: {}", new_data.device);
    match new_data.device.as_str() {
        "BST" => println!("String Potentiometer Enabled: {}",new_data.check),
        _ => println!("Check: {}",new_data.check),
    }

    let path = format!("/home/root/M4_Firmware/{}-Firmware/",new_data.abbrv_device());
    let script_path = Path::new(&path);
    let script = format!("./fw_cortex_m4.sh").to_owned();

    println!("Loading M4 firmware for device {}",new_data.abbrv_device());
    println!("at: {}",path);

    let output = Command::new(script)
        .current_dir(script_path)
        .arg("start")
        .output();


    /* Check if device exists, otherwise keep checking */
    loop {
        match fs::metadata("/dev/ttyRPMSG0") {
            Ok(_) => break,
            Err(_) => {},
        }
    }

    match output {
        Ok(result) => {
            println!("{}",String::from_utf8_lossy(&result.stdout));
            println!("Firmware loaded successfully!");

            match ipc_comm() {
                Ok(()) => (),
                Err(e) => println!("Error Opening:{}",e),
            };

        },
        Err(e) => {
            println!("Error loading firmware!: {}",e);
        }
    };

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

    println!("------Rust Server for Web Assembly Application-----");
    println!("---------------------------------------------------");
    println!("Server Listening http://localhost:8080\n");
    warp::serve(routes)
        .run(([172,20,10,7], 8080))
        .await;
}
