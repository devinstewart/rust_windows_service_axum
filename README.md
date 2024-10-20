# Rust Axum as a Windows Service
This is a simple example of how to run a Rust Axum web server as a Windows service using [windows_service](https://docs.rs/windows-service/latest/windows_service/) and have an MSI installer created using [cargo_wix](https://volks73.github.io/cargo-wix/cargo_wix/).

## My Setup
- Windows Server 2022
- Rust (Installed by first downloading and installing the [Microsoft C++ build tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/), then downloading and running `RUSTUP-INIT.exe`)
- [WiX Toolset 3.14.1](https://github.com/wixtoolset/wix3/releases/tag/wix3141rtm)
- `cargo-wix` (Installed by running `cargo install cargo-wix`)

## Building the MSI Installer
Run `cargo wix` or `cargo wix --nocapture` to build the MSI installer. The installer will be created in the `target\wix` directory.

The `--nocapture` flag will show the output of cargo and the WiX compiler and linker.

**Please Note:**

In the `Cargo.toml` file, this section is used to create the MSI installer:
```toml
[package.metadata.wix]
compiler-args = ["-ext", "WixFirewallExtension"]
linker-args = ["-ext", "WixFirewallExtension"]
```
This is needed to pass the `WixFirewallExtension` to the WiX compiler and linker to create the firewall rule.

If this section is not included, to build the MSI installer this is how to pass the `WixFirewallExtension` to the WiX compiler and linker:
```shell
cargo wix  -C -ext -C WixFirewallExtension -L -ext -L WixFirewallExtension
```

## Installing the MSI Installer
Run the MSI installer created in the `target\wix` directory. The installer will install the service and start it. The service will be set to start automatically on boot.

## My Results
First page of the installer:\
![Installer](screenshots/install.png)

Service installed and running:\
![Services](screenshots/services.png)

Firewall rule added:\
![Firewall](screenshots/firewall.png)

Using `curl` to test the web server from another machine:\
![Curl](screenshots/curl.png)

## Errata
I created this project out of a quick need to have a web server running as a Windows service. I spend most of my time in Linux, and this got me up and running quickly. If there are any issues or improvements, please let me know. I would be happy to update this project. Meanwhile, I hope this helps others who want to play in the Rust / Windows Service world.