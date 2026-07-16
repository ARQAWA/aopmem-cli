//! Invocation-scoped local desktop UI server.

mod assets;
pub(crate) mod data;
mod http;

use crate::output::OutputWarning;
use std::io;
use std::sync::Arc;
use thiserror::Error;

pub(crate) const UI_BROWSER_OPEN_FAILED: &str = "UI_BROWSER_OPEN_FAILED";

#[derive(Debug, Error)]
pub(crate) enum UiError {
    #[error(transparent)]
    Http(#[from] http::HttpError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UiOptions {
    port: u16,
    no_open: bool,
}

impl UiOptions {
    pub(crate) fn new(port: u16, no_open: bool) -> Self {
        Self { port, no_open }
    }
}

impl Default for UiOptions {
    fn default() -> Self {
        Self::new(0, false)
    }
}

pub(crate) trait BrowserLauncher {
    fn launch(&self, url: &str) -> io::Result<()>;
}

pub(crate) struct SystemBrowserLauncher;

impl BrowserLauncher for SystemBrowserLauncher {
    fn launch(&self, url: &str) -> io::Result<()> {
        launch_system_browser(url)
    }
}

pub(crate) struct StartedUi {
    server: http::HttpServer,
    url: String,
    warning: Option<OutputWarning>,
}

impl StartedUi {
    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn port(&self) -> u16 {
        self.server.address().port()
    }

    pub(crate) fn warning(&self) -> Option<&OutputWarning> {
        self.warning.as_ref()
    }

    pub(crate) fn serve(self) -> Result<(), UiError> {
        self.server.serve().map_err(UiError::from)
    }
}

pub(crate) fn start_with_launcher(
    options: UiOptions,
    context: data::UiDataContext,
    launcher: &dyn BrowserLauncher,
) -> Result<StartedUi, UiError> {
    let server =
        http::HttpServer::bind(http::BindConfig::loopback(options.port), Arc::new(context))?;
    let url = server.url();
    let warning = if options.no_open {
        None
    } else {
        launcher.launch(&url).err().map(|_| OutputWarning {
            code: UI_BROWSER_OPEN_FAILED,
            message: "browser open failed; the local UI server is still running at the printed URL"
                .to_string(),
        })
    };
    Ok(StartedUi {
        server,
        url,
        warning,
    })
}

#[cfg(target_os = "macos")]
fn launch_system_browser(url: &str) -> io::Result<()> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg(url)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("macOS browser launcher failed"))
    }
}

#[cfg(windows)]
fn launch_system_browser(url: &str) -> io::Result<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    const OPEN: [u16; 5] = [b'o' as u16, b'p' as u16, b'e' as u16, b'n' as u16, 0];
    let mut url = url.encode_utf16().collect::<Vec<_>>();
    if url.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "UI URL contains NUL",
        ));
    }
    url.push(0);
    // SAFETY: operation and URL are live NUL-terminated UTF-16 strings. No
    // command shell, PowerShell, or inherited mutable buffer is involved.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            OPEN.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize > 32 {
        Ok(())
    } else {
        Err(io::Error::other("Windows browser launcher failed"))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn launch_system_browser(_url: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "desktop browser launch is unsupported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_context() -> data::UiDataContext {
        let paths = crate::storage::resolve_paths().expect("test paths should resolve");
        data::UiDataContext::new(
            "ui-test-workspace".to_string(),
            crate::storage::workspace_paths_for_key(&paths, "ui-test-workspace"),
        )
    }

    struct FakeLauncher {
        calls: AtomicUsize,
        fail: bool,
    }

    impl FakeLauncher {
        fn successful() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail: true,
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl BrowserLauncher for FakeLauncher {
        fn launch(&self, _url: &str) -> io::Result<()> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if self.fail {
                Err(io::Error::other("forced browser launch failure"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn default_options_use_random_port_and_open_browser() {
        let options = UiOptions::default();
        assert_eq!(options.port, 0);
        assert!(!options.no_open);
    }

    #[test]
    fn no_open_skips_launcher_and_still_starts_loopback_server() {
        let launcher = FakeLauncher::successful();
        let started = start_with_launcher(UiOptions::new(0, true), test_context(), &launcher)
            .expect("UI should start without browser launch");

        assert_eq!(launcher.calls(), 0);
        assert!(started.warning().is_none());
        assert_ne!(started.port(), 0);
        assert!(started.url().starts_with("http://127.0.0.1:"));
        TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, started.port()))
            .expect("started UI listener should remain alive");
    }

    #[test]
    fn browser_failure_is_warning_and_server_remains_alive() {
        let launcher = FakeLauncher::failing();
        let started = start_with_launcher(UiOptions::default(), test_context(), &launcher)
            .expect("browser failure must not fail UI start");

        assert_eq!(launcher.calls(), 1);
        let warning = started.warning().expect("browser failure should warn");
        assert_eq!(warning.code, UI_BROWSER_OPEN_FAILED);
        assert!(!warning.message.contains(started.url()));
        assert!(!format!("{warning:?}").contains(
            started
                .url()
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .expect("UI URL should contain a token")
        ));
        TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, started.port()))
            .expect("UI listener should survive browser launch warning");
    }

    #[test]
    fn successful_launcher_is_called_once() {
        let launcher = FakeLauncher::successful();
        let started = start_with_launcher(UiOptions::default(), test_context(), &launcher)
            .expect("UI should start with successful launcher");

        assert_eq!(launcher.calls(), 1);
        assert!(started.warning().is_none());
    }
}
