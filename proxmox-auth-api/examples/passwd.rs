//! Test the `Pam` authenticator's 'store_password' implementation.

use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{bail, format_err, Error};

use proxmox_auth_api::api::Authenticator;
use proxmox_auth_api::types::Username;

static LOG: PrintLog = PrintLog;

fn main() -> Result<(), Error> {
    poll_result_once(run())
}

async fn run() -> Result<(), Error> {
    log::set_logger(&LOG).unwrap();
    log::set_max_level(log::LevelFilter::Debug);

    let mut args = std::env::args().skip(1);
    let (username, changepass): (Username, bool) = match args.next() {
        None => bail!("missing username or --check parameter"),
        Some(ck) if ck == "--check" => (
            args.next()
                .ok_or_else(|| format_err!("expected username as paramter"))?
                .try_into()?,
            false,
        ),
        Some(username) => (username.try_into()?, true),
    };

    let mut stdout = std::io::stdout();
    stdout.write_all(b"New password: ")?;
    stdout.flush()?;

    let mut input = std::io::stdin().lines();
    let password = input
        .next()
        .ok_or_else(|| format_err!("failed to read new password"))??;

    let realm = proxmox_auth_api::Pam::new("test");
    if changepass {
        realm.store_password(&username, &password)?;
    } else {
        realm.authenticate_user(&username, &password).await?;
    }

    Ok(())
}

struct PrintLog;

impl log::Log for PrintLog {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn flush(&self) {
        let _ = std::io::stdout().flush();
    }

    fn log(&self, record: &log::Record<'_>) {
        let _ = writeln!(std::io::stdout(), "{}", record.args());
    }
}

pub fn poll_result_once<T, R>(mut fut: T) -> Result<R, Error>
where
    T: Future<Output = Result<R, Error>>,
{
    let waker = std::task::RawWaker::new(std::ptr::null(), &WAKER_VTABLE);
    let waker = unsafe { std::task::Waker::from_raw(waker) };
    let mut cx = Context::from_waker(&waker);
    unsafe {
        match Pin::new_unchecked(&mut fut).poll(&mut cx) {
            Poll::Pending => bail!("got Poll::Pending synchronous context"),
            Poll::Ready(r) => r,
        }
    }
}

const WAKER_VTABLE: std::task::RawWakerVTable =
    std::task::RawWakerVTable::new(forbid_clone, forbid_wake, forbid_wake, ignore_drop);

unsafe fn forbid_clone(_: *const ()) -> std::task::RawWaker {
    panic!("tried to clone waker for synchronous task");
}

unsafe fn forbid_wake(_: *const ()) {
    panic!("tried to wake synchronous task");
}

unsafe fn ignore_drop(_: *const ()) {}
