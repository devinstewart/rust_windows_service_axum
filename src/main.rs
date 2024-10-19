extern crate windows_service;

use std::{ffi::OsString, io::{Error, ErrorKind::Other}};
use windows_service::{define_windows_service, 
    service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType}, 
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle}, service_dispatcher, Error::Winapi, Result
};
use tokio::{sync::mpsc, runtime::Runtime, net::TcpListener};
use axum::{Router, routing::get};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::{trace::TraceLayer, timeout::TimeoutLayer};
use std::time::Duration;

const SERVICE_NAME: &str = "Rust Axum Service";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

#[cfg(not(windows))]
fn main() {
    panic!("This program is only intended to be run on Windows.");
}

#[cfg(windows)]
fn main() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(_e) = run_service() {
        // As we are running as a service, we do not have access to stdout or stderr.
        // Errors can be written to a log file. 
    }
}

// All errors need to notify the service manager that the service is stopping before returning the error.
// As we do a lot of error handling, we will create a function to handle this with an exit code of 1.

fn notify_stop_and_return_error(status_handle: &ServiceStatusHandle, error: String) -> Result<()> {
    match status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(1),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    }) {
        Ok(_) => (),
        Err(e) => return Err(Winapi(Error::new(Other, format!("On Error: {}\nFailed to set service status: {:?}", error, e))))
    }

    Err(Winapi(Error::new(Other, error)))
}

fn run_service() -> Result<()> {
    // Create a channel to receive stop events.
    let (tx, rx) = mpsc::channel(1);
    
    // Define a closure to handle service events.
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                match tx.blocking_send(()) {
                    Ok(_) => (),
                    Err(_) => return ServiceControlHandlerResult::Other(1)
                }
                ServiceControlHandlerResult::NoError
            },
            ServiceControl::UserEvent(code) => {
                if code.to_raw() == 130 {
                    match tx.blocking_send(()) {
                        Ok(_) => (),
                        Err(_) => return ServiceControlHandlerResult::Other(1)
                    }
                }
                ServiceControlHandlerResult::NoError
            },
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register the service control handler.
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Create an async runtime to run the server.
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return notify_stop_and_return_error(&status_handle, format!("Failed to create runtime: {:?}", e))
    };

    // Start the async runtime.
    rt.block_on(async {

        // Notify the service manager that the service is running.
        match status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        }) {
            Ok(_) => (),
            Err(e) => return notify_stop_and_return_error(&status_handle, format!("Failed to set service status: {:?}", e))
        }

        // Enable tracing for graceful shutdown.
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    format!(
                        "{}axum=trace",
                        env!("CARGO_CRATE_NAME")
                    )
                    .into()
                }),
            )
            .with(tracing_subscriber::fmt::layer().without_time())
            .init();


        // Create the Axum app with tracing and timeout layers for graceful shutdown.
        let app = Router::new()
            .route("/", get(|| async { "Hello, From a Windows Service running Rust's Axum!\n" }))
            .layer((
                TraceLayer::new_for_http(),
                // Graceful shutdown will wait for outstanding requests to complete.
                // A timeout of 10 seconds is added to ensure the server does not hang indefinitely.
                TimeoutLayer::new(Duration::from_secs(10))
            ));

        let listener = match TcpListener::bind("0.0.0.0:3000").await {
            Ok(listener) => listener,
            Err(e) => return notify_stop_and_return_error(&status_handle, format!("Failed to bind to port: {:?}", e))
        };

        // Start the server with graceful shutdown that waits for the stop event.
        match axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(rx)).await {
            Ok(_) => (),
            Err(e) => return notify_stop_and_return_error(&status_handle, format!("Failed to start server: {:?}", e))
        }

        // Notify the service manager that the service is stopping.
        match status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        }) {
            Ok(_) => (),
            Err(e) => return notify_stop_and_return_error(&status_handle, format!("Failed to set service status: {:?}", e))
        }

        Ok(())
    })
}

async fn shutdown_signal(mut rx: mpsc::Receiver<()>) {
    let terminate = rx.recv();

    tokio::select! {
        _ = terminate => {},
    }
}