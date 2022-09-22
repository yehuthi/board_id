//! [`BoardId`]

#![forbid(missing_docs, unsafe_code)]
#[cfg(not(target_os = "linux"))] compile_error!("Only Linux is supported for the time being.");

use std::{io::{self, Read}, fs::File, fmt::Display, path::Path};


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
        /// Opens a file, returning `Ok(None)` if it doesn't exist.
        fn open_existing_file(path: impl AsRef<Path>) -> io::Result<Option<File>> {
            match File::open(path) {
                Ok(file) => Ok(Some(file)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        }
        macro_rules! dmi { ($part:expr) => { open_existing_file(concat!("/sys/class/dmi/id/board_", $part)) }; }
        Self::from_streams(dmi!("vendor")?, dmi!("name")?, dmi!("version")?)
    }

    /// Attempts to detect the [`BoardId`] from stream IDs.
    ///
    /// The streams are expected to have the format of the `/sys/class/dmi/id/board_*` files, i.e. contain just their
    /// respective part with a trailing NL.
    fn from_streams(vendor: Option<impl Read>, name: Option<impl Read>, version: Option<impl Read>) -> io::Result<Self> {
        let mut buffer = [0u8; 254];
        let mut buffer_write = buffer.as_mut_slice();

        fn read(buffer: &mut [u8], mut stream: impl Read) -> io::Result<usize> {
            let mut n = 0;
            loop {
                let m = stream.read(buffer)?;
                if m == 0 { break } else { n += m }
            }
            Ok(n.saturating_sub(1)) // remove trailing NL
        }

        let vendor = if let Some(vendor) = vendor {
            let read = read(buffer_write, vendor)?;
            buffer_write = &mut buffer_write[read..];
            read
        } else { 0 };

        let name = if let Some(name) = name {
            let read = read(buffer_write, name)?;
            buffer_write = &mut buffer_write[read..];
            read
        } else { 0 };

        let version = if let Some(version) = version {
            read(buffer_write, version)?
        } else { 0 };

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

        let detected_name = if let Some(name) = self.name() {
            write!(f, "{}", name.escape_ascii())?;
            true
        } else if wrote_vendor {
            write!(f, "motherboard")?;
            false
        } else {
            return write!(f, "undetected motherboard");
        };

        if detected_name {
            if let Some(version) = self.version() {
                write!(f, " {}", version.escape_ascii())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const NOENT: Option<&[u8]> = None;

    mod from_streams {
        use super::*;

        #[test]
        fn none() {
            let result = BoardId::from_streams(NOENT, NOENT, NOENT).unwrap();
            assert_eq!( result.vendor(), None);
            assert_eq!(   result.name(), None);
            assert_eq!(result.version(), None);
        }

        #[test]
        fn all_streams() {
            let result = BoardId::from_streams(Some("VENDOR\n".as_bytes()), Some("NAME\n".as_bytes()), Some("VERSION\n".as_bytes())).unwrap();
            assert_eq!( result.vendor(), Some("VENDOR" .as_bytes()));
            assert_eq!(   result.name(), Some("NAME"   .as_bytes()));
            assert_eq!(result.version(), Some("VERSION".as_bytes()));
        }

        #[test]
        fn no_version() {
            let result = BoardId::from_streams(Some("VENDOR\n".as_bytes()), Some("NAME\n".as_bytes()), None::<&[u8]>).unwrap();
            assert_eq!( result.vendor(), Some("VENDOR".as_bytes()));
            assert_eq!(   result.name(), Some("NAME"  .as_bytes()));
            assert_eq!(result.version(), None                     );
        }

        #[test]
        fn only_name() {
            let result = BoardId::from_streams(NOENT, Some("NAME\n".as_bytes()), NOENT).unwrap();
            assert_eq!( result.vendor(), None                   );
            assert_eq!(   result.name(), Some("NAME".as_bytes()));
            assert_eq!(result.version(), None                   );
        }
    }

    mod format {
        use super::*;

        #[test]
        fn undetected() {
            let board = BoardId::from_streams(NOENT, NOENT, NOENT).unwrap();
            assert_eq!(board.to_string(), "undetected motherboard");
        }

        #[test]
        fn only_vendor() {
            let board = BoardId::from_streams(Some("VENDOR\n".as_bytes()), NOENT, NOENT).unwrap();
            assert_eq!(board.to_string(), "VENDOR motherboard");
        }

        #[test]
        fn vendor_and_version() {
            let board = BoardId::from_streams(Some("VENDOR\n".as_bytes()), NOENT, Some("VERSION\n".as_bytes())).unwrap();
            assert_eq!(board.to_string(), "VENDOR motherboard");
        }

        #[test]
        fn only_version() {
            let board = BoardId::from_streams(NOENT, NOENT, Some("VERSION".as_bytes())).unwrap();
            assert_eq!(board.to_string(), "undetected motherboard");
        }

        #[test]
        fn full() {
            let board = BoardId::from_streams(Some("VENDOR\n".as_bytes()), Some("NAME\n".as_bytes()), Some("VERSION\n".as_bytes())).unwrap();
            assert_eq!(board.to_string(), "VENDOR NAME VERSION");
        }

        #[test]
        fn only_name() {
            let board = BoardId::from_streams(NOENT, Some("NAME\n".as_bytes()), NOENT).unwrap();
            assert_eq!(board.to_string(), "NAME");
        }
    }
}
