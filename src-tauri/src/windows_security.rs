use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenUIAccess, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub fn has_ui_access() -> bool {
    ui_access_from_result(query_ui_access())
}

fn ui_access_from_result(result: windows::core::Result<bool>) -> bool {
    result.unwrap_or(false)
}

fn query_ui_access() -> windows::core::Result<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)? };
    let result = (|| {
        let mut enabled = 0u32;
        let mut returned = 0u32;
        unsafe {
            GetTokenInformation(
                token,
                TokenUIAccess,
                Some((&raw mut enabled).cast()),
                std::mem::size_of::<u32>() as u32,
                &mut returned,
            )?;
        }
        Ok(enabled != 0)
    })();
    let _ = unsafe { CloseHandle(token) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn querying_the_current_token_succeeds() {
        assert!(query_ui_access().is_ok());
    }

    #[test]
    fn token_results_fail_closed() {
        assert!(ui_access_from_result(Ok(true)));
        assert!(!ui_access_from_result(Ok(false)));
        assert!(!ui_access_from_result(Err(
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
        )));
    }
}
