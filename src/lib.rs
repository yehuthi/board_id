//! [`BoardId`]

#![forbid(missing_docs, unsafe_code)]
#[cfg(not(target_os = "linux"))] compile_error!("Only Linux is supported for the time being.");

use std::{io::{self, Read}, fs::File, fmt::Display};


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
        let mut buffer = [0u8; 254];
        let mut buffer_write = buffer.as_mut_slice();

        fn read(buffer: &mut [u8], path: &str) -> io::Result<usize> {
            let mut n = 0;
            let mut file = match File::open(path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
                Err(e) => return Err(e),
            };
            loop {
                let m = file.read(buffer)?;
                if m == 0 { break } else { n += m }
            }
            Ok(n)
        }

        let mut vendor = read(buffer_write, "/sys/class/dmi/id/board_vendor")?;
        if vendor > 0 {
            vendor -= 1; // remove trailing NL
            buffer_write = &mut buffer_write[vendor..];
        }
        let mut name = read(buffer_write, "/sys/class/dmi/id/board_name")?;
        if name > 0 {
            name -= 1; // remove trailing NL
            buffer_write = &mut buffer_write[name..];
        }
        let mut version = read(buffer_write, "/sys/class/dmi/id/board_version")?;
        if version > 0 {
            version -= 1; // remove trailing NL
        }

        let vendor  = vendor           as u8;
        let name    = vendor + name    as u8;
        let version =   name + version as u8;
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
/// - Has the format "<vendor> <name> <version>".
/// - If it has vendor without name it will say "<vendor> motherboard".
/// - Version is only shown if the name is detected (to avoid unhelpful messages like "undetected motherboard 1.0").
/// - If no part has been detected it will say "undetected motherboard"
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
