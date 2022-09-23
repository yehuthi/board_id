//! [`BoardId`] detection.

#![forbid(missing_docs, unsafe_code)]
#[cfg(not(target_os = "linux"))] compile_error!("Only Linux is supported for the time being.");

use std::{io::{self, Read}, fs::File, fmt::Display, path::Path};

/// Motherboard ID.
#[derive(Debug, Hash, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct BoardId {
    /// The data buffer.
    buffer: [u8; Self::BUFSZ],
    /// The exclusive end for the vendor part.
    vendor: u8,
    /// The exclusive end for the name part.
    name: u8,
    /// The exclusive end for the version part.
    version: u8,
}

impl BoardId {
    /// The buffer size.
    const BUFSZ: usize = u8::MAX as usize;

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
    /// The streams are expected to have the format of the `/sys/class/dmi/id/board_*` files,
    /// i.e. contain just their respective part with a trailing NL.
    ///
    /// There's no intermediary buffer for efficiency, so this makes the buffer size effectively
    /// be `Self::BUFSZ - 2`, which hardly matters.
    /// The buffer size is reduced by two because when the input exactly fits, the last byte is
    /// going to be NL. And we need a spare byte so we'll have room to read and see if we reached
    /// EOF.
    fn from_streams(vendor: Option<impl Read>, name: Option<impl Read>, version: Option<impl Read>) -> io::Result<Self> {
        let mut buffer = [0u8; Self::BUFSZ];

        fn read(buffer: &mut [u8], mut stream: impl Read) -> io::Result<usize> {
            let mut n = 0;
            loop {
                let buf = &mut buffer[n..];
                if buf.is_empty() {
                    return Err(io::Error::new(io::ErrorKind::WriteZero, "the motherboard ID is abnormally large and doesn't fit in the buffer"))
                }
                let m = stream.read(buf)?;
                if m == 0 { break } else { n += m }
            }
            Ok(n.saturating_sub(1)) // remove trailing NL
        }

        let vendor_count  = vendor .map_or(Ok(0), |r| read(&mut buffer,                              r))?;
        let name_count    = name   .map_or(Ok(0), |r| read(&mut buffer[vendor_count..],              r))?;
        let version_count = version.map_or(Ok(0), |r| read(&mut buffer[vendor_count + name_count..], r))?;

        Ok(Self {
            buffer,
             vendor:  vendor_count                               as u8,
               name: (vendor_count + name_count                ) as u8,
            version: (vendor_count + name_count + version_count) as u8,
        })
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
        fn empty_streams() {
            let result = BoardId::from_streams(Some("\n".as_bytes()), Some("\n".as_bytes()), Some("\n".as_bytes())).unwrap();
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

        mod buffer {
            use super::*;

            #[test]
            fn name_too_large() {
                let name = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Pellentesque ut nisi dignissim, sodales leo id, euismod dolor. Curabitur justo sem, aliquam aliquet purus ut, feugiat sagittis justo. Curabitur vel lobortis tortor. Vivamus at porttitor mi eleifend";
                assert_eq!(name.len(), BoardId::BUFSZ - 1, "bad test");
                let e = BoardId::from_streams(NOENT, Some(format!("{name}\n").as_bytes()), NOENT).unwrap_err();
                assert_eq!(e.kind(), io::ErrorKind::WriteZero);
            }

            #[test]
            fn name_exact_fit() {
                let name = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Pellentesque ut nisi dignissim, sodales leo id, euismod dolor. Curabitur justo sem, aliquam aliquet purus ut, feugiat sagittis justo. Curabitur vel lobortis tortor. Vivamus at portitor mi eleifend";
                assert_eq!(name.len(), BoardId::BUFSZ - 2, "bad test");
                let board = BoardId::from_streams(NOENT, Some(format!("{name}\n").as_bytes()), NOENT).unwrap();
                assert_eq!( board.vendor(), None                 );
                assert_eq!(   board.name(), Some(name.as_bytes()));
                assert_eq!(board.version(), None                 );
            }
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
