use std::ffi::{c_int, c_void, CStr, CString};
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;

use anyhow::{bail, format_err, Error};
use pam_sys::types::{
    PamHandle, PamItemType, PamMessage, PamMessageStyle, PamResponse, PamReturnCode,
};

use crate::types::UsernameRef;

#[allow(clippy::upper_case_acronyms)]
pub struct Pam {
    service: &'static str,
}

impl Pam {
    pub const fn new(service: &'static str) -> Self {
        Self { service }
    }
}

impl crate::api::Authenticator for Pam {
    fn authenticate_user<'a>(
        &'a self,
        username: &'a UsernameRef,
        password: &'a str,
        client_ip: Option<&'a IpAddr>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            let mut password_conv = PasswordConv {
                login: username.as_str(),
                password,
            };

            let conv = pam_sys::types::PamConversation {
                conv: Some(conv_fn),
                data_ptr: &mut password_conv as *mut _ as *mut c_void,
            };

            let mut handle = std::ptr::null_mut();
            let err =
                pam_sys::wrapped::start(self.service, Some(username.as_str()), &conv, &mut handle);
            if err != PamReturnCode::SUCCESS {
                bail!("error opening pam - {err}");
            }
            let mut handle = PamGuard {
                handle: unsafe { &mut *handle },
                result: PamReturnCode::SUCCESS,
            };

            if let Some(ip) = client_ip {
                let ip = ip.to_string();
                let ip = CString::new(ip).map_err(|_| format_err!("nul-byte in client ip"))?;
                let ip = unsafe { &*(ip.as_ptr() as *const libc::c_void) };

                let err = pam_sys::wrapped::set_item(handle.handle, PamItemType::RHOST, ip);
                if err != PamReturnCode::SUCCESS {
                    bail!("error setting PAM_RHOST - {err}");
                }
            }

            handle.result =
                pam_sys::wrapped::authenticate(handle.handle, pam_sys::types::PamFlag::NONE);
            if handle.result != PamReturnCode::SUCCESS {
                bail!("authentication error - {err}");
            }

            Ok(())
        })
    }

    fn store_password(
        &self,
        username: &UsernameRef,
        password: &str,
        client_ip: Option<&IpAddr>,
    ) -> Result<(), Error> {
        let mut password_conv = PasswordConv {
            login: username.as_str(),
            password,
        };

        let conv = pam_sys::types::PamConversation {
            conv: Some(conv_fn),
            data_ptr: &mut password_conv as *mut _ as *mut c_void,
        };

        let mut handle = std::ptr::null_mut();
        let err =
            pam_sys::wrapped::start(self.service, Some(username.as_str()), &conv, &mut handle);
        if err != PamReturnCode::SUCCESS {
            bail!("error opening pam - {err}");
        }
        let mut handle = PamGuard {
            handle: unsafe { &mut *handle },
            result: PamReturnCode::SUCCESS,
        };

        if let Some(ip) = client_ip {
            let ip = ip.to_string();
            let ip = CString::new(ip).map_err(|_| format_err!("nul-byte in client ip"))?;
            let ip = unsafe { &*(ip.as_ptr() as *const libc::c_void) };

            let err = pam_sys::wrapped::set_item(handle.handle, PamItemType::RHOST, ip);
            if err != PamReturnCode::SUCCESS {
                bail!("error setting PAM_RHOST - {err}");
            }
        }

        /*
         * we assume we're root and don't need to authenticate
        handle.result =
            pam_sys::wrapped::authenticate(handle.handle, pam_sys::types::PamFlag::NONE);
        if handle.result != PamReturnCode::SUCCESS {
            bail!("authentication error - {err}");
        }

        handle.result = pam_sys::wrapped::acct_mgmt(handle.handle, pam_sys::types::PamFlag::NONE);
        if handle.result != PamReturnCode::SUCCESS {
            bail!("account error - {}", handle.result);
        }
        */

        handle.result = pam_sys::wrapped::chauthtok(handle.handle, pam_sys::types::PamFlag::NONE);
        if handle.result != PamReturnCode::SUCCESS {
            bail!("error changing auth token - {}", handle.result);
        }

        Ok(())
    }

    // do not remove password for pam users
    fn remove_password(&self, _username: &UsernameRef) -> Result<(), Error> {
        Ok(())
    }
}

extern "C" fn conv_fn(
    num_messages: c_int,
    messages: *mut *mut PamMessage,
    responses_out: *mut *mut PamResponse,
    data_ptr: *mut c_void,
) -> c_int {
    let messages: &[&PamMessage] = unsafe {
        std::slice::from_raw_parts(
            messages as *const *const PamMessage as *const &PamMessage,
            num_messages as usize,
        )
    };

    let mut responses = Vec::new();
    responses.resize(
        messages.len(),
        PamResponse {
            resp: std::ptr::null_mut(),
            resp_retcode: 0,
        },
    );
    let mut responses = responses.into_boxed_slice();

    let data_ptr = unsafe { &*(data_ptr as *const PasswordConv<'_>) };

    match data_ptr.converse(messages, &mut responses) {
        Ok(()) => {
            unsafe {
                std::ptr::write(responses_out, &mut Box::leak(responses)[0]);
            }
            PamReturnCode::SUCCESS as c_int
        }
        Err(err) => {
            log::error!("error conversing with pam - {err}");
            PamReturnCode::ABORT as c_int
        }
    }
}

struct PamGuard<'a> {
    handle: &'a mut PamHandle,
    result: PamReturnCode,
}

impl Drop for PamGuard<'_> {
    fn drop(&mut self) {
        pam_sys::wrapped::end(&mut self.handle, self.result);
    }
}

struct PasswordConv<'a> {
    login: &'a str,
    password: &'a str,
}

impl PasswordConv<'_> {
    fn converse(
        &self,
        messages: &[&PamMessage],
        responses: &mut [PamResponse],
    ) -> Result<(), Error> {
        for i in 0..messages.len() {
            self.msg(messages[i], &mut responses[i])?;
        }
        Ok(())
    }

    fn msg(
        &self,
        msg: &pam_sys::types::PamMessage,
        response: &mut PamResponse,
    ) -> Result<(), Error> {
        let resp = match PamMessageStyle::from(msg.msg_style) {
            PamMessageStyle::PROMPT_ECHO_ON => {
                //let msg = unsafe { CStr::from_ptr(msg.msg) };
                //log::info!("pam prompt: {msg:?}");
                self.login
            }
            PamMessageStyle::PROMPT_ECHO_OFF => {
                //let msg = unsafe { CStr::from_ptr(msg.msg) };
                //log::info!("pam password prompt: {msg:?}");
                self.password
            }
            PamMessageStyle::ERROR_MSG => {
                let msg = unsafe { CStr::from_ptr(msg.msg) };
                log::error!("pam error: {msg:?}");
                return Ok(());
            }
            PamMessageStyle::TEXT_INFO => {
                let msg = unsafe { CStr::from_ptr(msg.msg) };
                log::info!("pam message: {msg:?}");
                return Ok(());
            }
        };

        // Since CString::into_raw is technically not `free()`-safe...
        let resp = resp.as_bytes();
        let c_resp = unsafe { libc::malloc(resp.len() + 1) as *mut u8 };
        if c_resp.is_null() {
            bail!("failed to allocate response");
        }
        let c_resp = unsafe { std::slice::from_raw_parts_mut(c_resp, resp.len() + 1) };
        c_resp[c_resp.len() - 1] = 0;
        c_resp[..resp.len()].copy_from_slice(resp);
        response.resp = c_resp.as_mut_ptr() as *mut libc::c_char;
        Ok(())
    }
}
