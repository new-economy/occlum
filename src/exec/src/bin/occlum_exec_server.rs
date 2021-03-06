extern crate futures;
extern crate grpc;
extern crate libc;
extern crate occlum_exec;
extern crate protobuf;
#[macro_use]
extern crate log;
use futures::executor;
use grpc::prelude::*;
use grpc::ClientConf;
use occlum_exec::occlum_exec::HealthCheckRequest;
use occlum_exec::occlum_exec_grpc::{OcclumExecClient, OcclumExecServer};
use occlum_exec::server::OcclumExecImpl;
use occlum_exec::{DEFAULT_SERVER_FILE, DEFAULT_SOCK_FILE};
use std::env;
use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, Condvar, Mutex};

//Checks the server status, if the server is running return true, else recover the socket file and return false.
fn check_server_status(sock_file: &str) -> bool {
    if let Err(e) = std::fs::File::open(sock_file) {
        debug!("failed to open the sock_file {:?}", e);

        if e.kind() == std::io::ErrorKind::NotFound {
            return false;
        }
    }

    let client = OcclumExecClient::new_plain_unix(sock_file, ClientConf::new())
        .expect("failed to create UDS client");

    let resp = executor::block_on(
        client
            .status_check(
                grpc::RequestOptions::new(),
                HealthCheckRequest {
                    ..Default::default()
                },
            )
            .join_metadata_result(),
    );

    if let Ok(_) = resp {
        debug!("another server is running.");
        true
    } else {
        debug!("delete the useless socket file.");
        std::fs::remove_file(sock_file).expect("could not remove socket file");
        false
    }
}

fn main() {
    //get the UDS file name
    let args: Vec<String> = env::args().collect();
    let mut sockfile = String::from(args[0].as_str());
    let sockfile = str::replace(
        sockfile.as_mut_str(),
        DEFAULT_SERVER_FILE,
        DEFAULT_SOCK_FILE,
    );

    //If the server already startted, then return
    if check_server_status(sockfile.as_str()) {
        println!("server stared");
        return;
    }

    let server_stopped = Arc::new((Mutex::new(true), Condvar::new()));

    let service_def = OcclumExecServer::new_service_def(
        OcclumExecImpl::new_and_save_execution_lock(server_stopped.clone()),
    );
    let mut server_builder = grpc::ServerBuilder::new_plain();
    server_builder.add_service(service_def);
    match server_builder.http.set_unix_addr(sockfile) {
        Ok(_) => {}
        Err(e) => {
            debug!("{:?}", e);
            return;
        }
    };

    if let Ok(server) = server_builder.build() {
        rust_occlum_pal_init().expect("Occlum image initialization failed");
        //server is running
        println!("server stared on addr {}", server.local_addr());
        let (lock, cvar) = &*server_stopped;
        let mut server_stopped = lock.lock().unwrap();
        *server_stopped = false;
        while !*server_stopped {
            server_stopped = cvar.wait(server_stopped).unwrap();
        }
        rust_occlum_pal_destroy().expect("Destory occlum image failed");
        println!("server stopped");
    }
}

extern "C" {
    /*
     * @brief Initialize an Occlum enclave
     *
     * @param attr  Mandatory input. Attributes for Occlum.
     *
     * @retval If 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_init(attr: *const occlum_pal_attr_t) -> i32;

    /*
     * @brief Destroy the Occlum enclave
     *
     * @retval if 0, then success; otherwise, check errno for the exact error type.
     */
    fn occlum_pal_destroy() -> i32;
}

#[repr(C)]
/// Occlum PAL attributes. Defined by occlum pal.
pub struct occlum_pal_attr_t {
    /// Occlum instance dir.
    ///
    /// Specifies the path of an Occlum instance directory. Usually, this
    /// directory is initialized by executing "occlum init" command, which
    /// creates a hidden directory named ".occlum/". This ".occlum/" is an
    /// Occlum instance directory. The name of the directory is not necesarrily
    /// ".occlum"; it can be renamed to an arbitrary name.
    ///
    /// Mandatory field. Must not be NULL.
    pub instance_dir: *const libc::c_char,
    /// Log level.
    ///
    /// Specifies the log level of Occlum LibOS. Valid values: "off", "error",
    /// "warn", "info", and "trace". Case insensitive.
    ///
    /// Optional field. If NULL, the LibOS will treat it as "off".
    pub log_level: *const libc::c_char,
}

/// Loads and initializes the Occlum enclave image
fn rust_occlum_pal_init() -> Result<(), i32> {
    let mut instance_dir = OsString::from("./.occlum\0");
    if let Some(val) = env::var_os("OCCLUM_INSTANCE_DIR") {
        instance_dir = val;
        instance_dir.push("\0");
    };

    let mut log_level = OsString::from("off\0");
    if let Some(val) = env::var_os("OCCLUM_LOG_LEVEL") {
        log_level = val;
        log_level.push("\0");
    };
    debug!("{:?} {:?}", instance_dir, log_level);

    let occlum_pal_attribute = occlum_pal_attr_t {
        instance_dir: CStr::from_bytes_with_nul(instance_dir.as_bytes())
            .unwrap()
            .as_ptr(),
        log_level: CStr::from_bytes_with_nul(log_level.as_bytes())
            .unwrap()
            .as_ptr(),
    };
    let rust_object = Box::new(&occlum_pal_attribute);

    let ret = unsafe { occlum_pal_init(*rust_object) };
    match ret {
        0 => Ok(()),
        _ => Err(ret),
    }
}

///Destroyes the Occlum enclave image
fn rust_occlum_pal_destroy() -> Result<(), i32> {
    let ret = unsafe { occlum_pal_destroy() };
    match ret {
        0 => Ok(()),
        _ => Err(ret),
    }
}
