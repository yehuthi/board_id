//! [`BoardId`]

use std::{io::{self, Read, Write, IoSlice}, fs::File, fmt::Display};

#[cfg(not(target_os = "linux"))] compile_error!("Only Linux is supported for the time being.");
#[forbid(missing_docs, unsafe_code)]

/// Motherboard ID.
#[derive(Debug, Hash, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct BoardId {
    /// The data buffer.
    buffer: [u8; 254],
    /// The count of bytes for the vendor part.
    vendor: u8,
    /// The exclusive end for the name part.
    name: u8,
    /// The exclusive end for the version part.
    version: u8,
}

impl BoardId {
    /// Attempts to detect the [`BoardId`].
    pub fn detect() -> io::Result<Self> {
        let mut content = [0;512];
        let content = {
            let read = File::open("/sys/devices/virtual/dmi/id/modalias")?.read(&mut content)?;
            &content[..read]
        };

        // https://elixir.bootlin.com/linux/latest/source/drivers/firmware/dmi-id.c#L92
        const BOARD_VENDOR : [u8;3] = *b"rvn";
        const BOARD_NAME   : [u8;2] = *b"rn" ;
        const BOARD_VERSION: [u8;3] = *b"rvr";

        let mut vendor  = Default::default();
        let mut name    = Default::default();
        let mut version = Default::default();

        for part in content.split(|&b| b == b':') {
            if part.starts_with(&BOARD_VENDOR) {
                vendor = &part[BOARD_VENDOR.len()..];
            } else if part.starts_with(&BOARD_NAME) {
                name = &part[BOARD_NAME.len()..];
            } else if part.starts_with(&BOARD_VERSION) {
                version = &part[BOARD_VERSION.len()..];
            }
        }

        let mut buffer = [0u8; 254];
        let version = buffer.as_mut_slice().write_vectored(&[IoSlice::new(vendor), IoSlice::new(name), IoSlice::new(version)])? as u8;
        let vendor = vendor.len() as u8;
        let name = vendor + name.len() as u8;
        Ok(Self { buffer, vendor, name, version })
    }

    /// Gets the board's vendor / brand.
    #[inline]
    pub fn vendor(&self) -> Option<&[u8]> {
        (self.vendor > 0).then_some(&self.buffer[..self.vendor as usize])
    }

    /// Gets the board's name / model.
    #[inline]
    pub fn name(&self) -> Option<&[u8]> {
        (self.vendor != self.name).then_some(&self.buffer[self.vendor as usize..self.name as usize])
    }

    /// Gets the board's version.
    #[inline]
    pub fn version(&self) -> Option<&[u8]> {
        (self.name < self.version).then_some(&self.buffer[self.name as usize..self.version as usize])
    }
}

/// Semi-opinionated Display implementation.
///
/// It only displays a part if the previous part was detected:
/// - If we know every part it will say "{vendor} {name} {version}"
/// - If we don't know the version it will say "{vendor} {name}"
/// - If we don't know the name it will say "{vendor} motherboard"
/// - If we don't know anything it will say "undetected motherboard"
///
/// The rationale is that it's somewhat silly and rarely helpful to say things
/// like "undetected motherboard 1.0".
///
/// All the [`MotherboardId`] field are assumed to be valid ASCII. Invalid ASCII characters will be
/// escaped.
impl Display for BoardId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let wrote_vendor = if let Some(vendor) = self.vendor() {
            write!(f, "{} ", vendor.escape_ascii())?;
            true
        } else { false };

        if let Some(name) = self.name() {
            write!(f, "{}", name.escape_ascii())?;
        } else if wrote_vendor {
            write!(f, "motherboard")?;
        } else {
            return write!(f, "undetected motherboard");
        }

        if let Some(version) = self.version() {
            write!(f, " {}", version.escape_ascii())?;
        }

        Ok(())
    }
}
