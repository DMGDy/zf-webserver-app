use warp::Filter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::fs;
use std::fs::OpenOptions;
use std::error::Error;
use std::io::{BufReader,BufWriter,Write,Read};
use std::os::unix::fs::OpenOptionsExt;
use std::str;
use std::{thread, time::{Instant,Duration}};
use libc;
use tokio::sync::Mutex;


const VIRT_DEVICE: &str = "/dev/ttyRPMSG0";

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

fn rpmsg_read() -> Result<String, Box<dyn Error>> {
    let mut response_buff :Vec<u8> = Vec::new();

    let dev_rpmsg = OpenOptions::new()
        .read(true)
        .write(false)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(VIRT_DEVICE)?;

    let mut reader = BufReader::new(&dev_rpmsg);

    let start_time = Instant::now();
    let timeout = Duration::from_secs(1);
    let delta = Duration::from_millis(10);


    println!("Attempting to read from device...");
    while  start_time.elapsed() < timeout{
        match reader.read_to_end(&mut response_buff) {
            Ok(0) => { },
            Ok(_) => { break },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                /* ignore this error maybe if it gets here */
                if response_buff.is_empty() {
                    continue;
                }
                else { break }
            },
            Err(e) => {
                println!("Error reading device file!: {}",e);
                return Err(Box::new(e));
            }
        }
        thread::sleep(delta);
    }

    Ok(String::from_utf8(response_buff)?)
}

fn rpmsg_write(msg: &str) -> Result<(), Box<dyn Error>> {

    let dev_rpmsg = OpenOptions::new()
        .read(false)
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(VIRT_DEVICE)?;

    let mut rpmsg_writer = BufWriter::new(&dev_rpmsg);

    match rpmsg_writer.write(msg.as_bytes()) {
        Ok(_) => {
            return Ok(())
        },
        Err(e)=> {
            println!("Error Writing to device file: {}!",e);
            return Err(Box::new(e))
        },
    }
}

fn load_firmware(dev: &str) -> Result<Output, Box<dyn Error>> {
    let path = format!("/home/root/M4_Firmware/{}-Firmware/",dev);
    let script_path = Path::new(&path);
    let script = format!("./fw_cortex_m4.sh").to_owned();

    let output = Command::new(script)
        .current_dir(script_path)
        .arg("start")
        .output()?;

    loop {
        match fs::metadata(VIRT_DEVICE) {
            Ok(_) => break,
            Err(_) => {thread::sleep(Duration::from_millis(1))},
        }
    }

    Ok(output)
}

// handle POST req
fn handle_post(new_data: TestData, data_store: Arc<Mutex<Vec<TestData>>>) -> impl warp::Reply {
    println!("Test Info:\n\tDevice: {}", new_data.device);
    match new_data.device.as_str() {
        "BST" => println!("\tString Potentiometer Enabled: {}",new_data.check),
        _ => println!("\tCheck: {}",new_data.check),
    }

    let output = load_firmware(new_data.abbrv_device());

    match output {
        Ok(result) => {
            print!("{}",String::from_utf8_lossy(&result.stdout));
            println!("Firmware loaded successfully!");
        },
        Err(e) => {
            println!("Error loading firmware!: {}",e);
            println!("Server closing...");
            std::process::exit(-1)
        }
    };

    let msg = "test";
    match rpmsg_write(msg) {
        Ok(_) => {
            println!("Message < {} > written successfully!", msg);
        },
        Err(e) => {
            println!("Failed to open < {} > device file!: {}",VIRT_DEVICE,e);
            std::process::exit(-1)
        },
    }

     match rpmsg_read() {
        Ok(response) => {
            println!("Received response from device file:\n{}",response);
        },
        Err(e) => {
            println!("Failed to open < {} > device file!: {}", VIRT_DEVICE,e);
            std::process::exit(-1)
        }
    }

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
