use std::{io::Read, sync::OnceLock};

/// The SecureBoot status
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecureBoot {
    /// SecureBoot is enabled
    Enabled,
    /// SecureBoot is disabled
    Disabled,
}

/// The possible BootModes
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootMode {
    /// The BootMode is EFI/UEFI
    Efi,
    /// The BootMode is Legacy BIOS
    Bios,
}

static BOOT_MODE: OnceLock<BootMode> = OnceLock::new();
static SECURE_BOOT: OnceLock<SecureBoot> = OnceLock::new();

impl BootMode {
    /// Returns the current bootmode (BIOS or EFI)
    pub fn query() -> BootMode {
        *BOOT_MODE.get_or_init(|| {
            if std::path::Path::new("/sys/firmware/efi").exists() {
                BootMode::Efi
            } else {
                BootMode::Bios
            }
        })
    }
}

impl SecureBoot {
    /// Checks if secure boot is enabled
    pub fn query() -> SecureBoot {
        *SECURE_BOOT.get_or_init(|| {
            // Check if SecureBoot is enabled
            // Attention: this file is not seekable!
            // Spec: https://uefi.org/specs/UEFI/2.10/03_Boot_Manager.html?highlight=8be4d#globally-defined-variables
            let mut buf = [0; 5];
            if std::fs::File::open(
                "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c",
            )
            .and_then(|mut file| file.read_exact(&mut buf))
            .is_ok()
                && buf[4] == 1
            {
                SecureBoot::Enabled
            } else {
                SecureBoot::Disabled
            }
        })
    }
}
