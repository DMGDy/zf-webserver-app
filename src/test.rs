use std::{
    fs,
    fs::OpenOptions,
    process::{Command, Output},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    error::Error,
    io::{Write, Read},
    str,
    thread,
    time::{Instant,Duration},
};
use serde::{Serialize,Deserialize};
use colored::*;

pub const VIRT_DEVICE: &str = "/dev/ttyRPMSG0";

// struct expected from first POST request
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TestData {
    device: String,
    check: bool,
}

pub enum FimwareOption {
    START,
    STOP,
}

#[derive(Serialize)]
pub enum State{
    Awake,
    InProgress,
    Done,
    Idle,
    ENoFirmware,
    ENoRead,
    ENoWrite,
    EOpen,
}

impl State {
    pub fn code(&self) -> i32 {
        match self {
            Self::Idle=>0,
            Self::Awake=>1,
            Self::InProgress=>2,
            Self::Done=>3,
            Self::ENoFirmware=>-1,
            Self::ENoRead=>-2,
            Self::ENoWrite=>-3,
            Self::EOpen=>-4,
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




pub fn m4_firmware(dev: &str, option: FimwareOption) -> Result<Output, Box<dyn Error>> {
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



fn rpmsg_comm(msg: &str) -> Result<String, Box<dyn Error>> {

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
                        return Err(Box::new(e)); }
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

/**
 * begin_test - starts new device test
 * 
 * @params json from HTTP POST request as TestData struct
 */
pub fn begin_test(test_data: &TestData) -> State{
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
            return State::ENoFirmware;
        }
    };

    println!("---------------------------------------------------");

    let msg = "hello\n";
    match rpmsg_comm(msg) {
        Ok(response) => {
            println!("{}\n\t{}{}"
                ,"Message".green() ,msg,"written successfully!".green());
            println!("{}\n\t{}"
                ,"Response was:".cyan(),response);
        },
        Err(e) => {
            println!("{} {} {}: {}"
                ,"Failed to open".red(),VIRT_DEVICE
                ,"device file!".red().bold(),e);
            return State::EOpen
        },
    }

    println!("---------------------------------------------------");

    match rpmsg_comm(msg_bool) {
        Ok(response) => {
            println!("{}\n\t{}{}"
                ,"Message".green() ,msg_bool,"written successfully!".green());

            println!("{}\n\t{}"
                ,"Response was:".cyan(),response);
        },
        Err(e) => {
            println!("{} {} {}: {}"
                ,"Failed to open".red(),VIRT_DEVICE
                ,"device file!".red().bold(),e);
            return State::EOpen
        },
    }
    println!("---------------------------------------------------");
    // Test has started by this point

    State::Awake
}
