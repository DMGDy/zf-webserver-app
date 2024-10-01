use warp::Filter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::fs;
use std::fs::OpenOptions;
use std::error::Error;
use std::io::{Write,Read};
use std::os::unix::fs::OpenOptionsExt;
use std::str;
use std::{thread, time::{Instant,Duration}};
use colored::*;
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

#[derive(Serialize)]
enum State{
    Awake,
    InProgress,
    Done,
    Error,
    Idle,
}

enum FimwareOption {
    START,
    STOP,
}

impl State {
    fn code(&self) -> i32 {
        match self {
            Self::Idle=>0,
            Self::Awake=>1,
            Self::InProgress=>2,
            Self::Done=>3,
            Self::Error=>4,
        }
    }
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

impl FimwareOption {
    fn arg(&self) -> &str {
        match self {
            Self::START => "start",
            Self::STOP => "stop",
        }
    }
}

fn rpmsg_write(msg: &str) -> Result<String, Box<dyn Error>> {

    let mut dev_rpmsg = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(VIRT_DEVICE)?;

    let mut response_buff = Vec::new();

    let timeout = Duration::from_secs(1);
    let delta = Duration::from_millis(50);


    match dev_rpmsg.write(msg.as_bytes()) {
        Ok(_) => {
            println!("Attempting to read from device...");
            let start_time = Instant::now();
            while  start_time.elapsed() < timeout{
                match dev_rpmsg.read_to_end(&mut response_buff) {
                    Ok(0) => { 
                        if !response_buff.is_empty(){ break }
                    },
                    Ok(_) => { break },
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        /* ignore this rust stdio line bufferingerror maybe if it gets here */
                        if !response_buff.is_empty() { break }
                    },
                    Err(e) => {
                        println!("Error reading device file!: {}",e);
                        return Err(Box::new(e));
                    }
                }
               // print!("{}",String::from_utf8_lossy(&response_buff));
                thread::sleep(delta);
            }
        },
        Err(e)=> {
            println!("Error Writing to device file: {}!",e);
            return Err(Box::new(e))
        },
    }
    Ok(String::from_utf8(response_buff)?)

}

fn m4_firmware(dev: &str, option: FimwareOption) -> Result<Output, Box<dyn Error>> {
    let path = format!("/home/root/M4_Firmware/{}-Firmware/",dev);
    let script_path = Path::new(&path);
    let script = format!("./fw_cortex_m4.sh").to_owned();

    let output = Command::new(script)
        .current_dir(script_path)
        .arg(option.arg())
        .output()?;

    match option {
        FimwareOption::START => {
            loop {
                match fs::metadata(VIRT_DEVICE) {
                    Ok(_) => break,
                    Err(_) => {thread::sleep(Duration::from_millis(1))},
                }
            }
        }
        FimwareOption::STOP =>{
            return Ok(output);
        }
    }
    Ok(output)
}

/**
 * begin_test - starts new device test
 * 
 * @params json from HTTP POST request as TestData struct
 */

fn begin_test(test_data: &TestData) -> State{
    println!("{}\n{{\n\t{}: {}",
        "Test Info".bold(),"Device".underline(),test_data.device.bold());
    let mut msg_bool = "no\n";

    match test_data.abbrv_device() {
        "BST" => {
            println!("\t{}: {}\n}}",
                "String Potentiometer Enabled".underline()
                ,test_data.check.to_string().bold());
            match test_data.check {
                true => msg_bool = "yes\n",
                false => msg_bool= "no\n",
            }
        },
        _ => println!("\tCheck: {}",test_data.check),
    };
    let output = m4_firmware(test_data.abbrv_device(),FimwareOption::START);

    match output {
        Ok(result) => {
            print!("{}",String::from_utf8_lossy(&result.stdout));
            println!("{}","Firmware loaded successfully!".green());
        },
        Err(e) => {
            println!("{} {}{}: {}",
                "Error loading firmware for device".red().bold(),
                test_data.abbrv_device(),"!".red().bold(),e);
            return State::Error;
        }
    };

    println!("---------------------------------------------------");
    let msg = "hello\n";
    match rpmsg_write(msg) {
        Ok(response) => {
            println!("{}\n{{\n\t{}}}\n{}"
                ,"Message".green() ,msg,"written successfully!".green());
            println!("{}\n{{\n\n\t{}}}\n"
                ,"Response was:".cyan(),response);
        },
        Err(e) => {
            println!("{} {} {}: {}"
                ,"Failed to open".red(),VIRT_DEVICE
                ,"device file!".red().bold(),e);
            return State::Error
        },
    }

    println!("---------------------------------------------------");
    match rpmsg_write(msg_bool) {
        Ok(response) => {
            println!("{}\n{{\n\t{}}}\n{}"
                ,"Message".green() ,msg_bool,"written successfully!".green());

            println!("{}\n{{\n\n\t{}}}\n"
                ,"Response was:".cyan(),response);
        },
        Err(e) => {
            println!("{} {} {}: {}"
                ,"Failed to open".red(),VIRT_DEVICE
                ,"device file!".red().bold(),e);
            return State::Error
        },
    }

    println!("---------------------------------------------------");

    State::Awake
}

// handle POST req
fn handle_post(new_data: TestData, data_store: Arc<Mutex<Vec<TestData>>>) -> impl warp::Reply {
    let response = begin_test(&new_data);
   
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
   match m4_firmware(new_data.abbrv_device(),FimwareOption::STOP) {
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
