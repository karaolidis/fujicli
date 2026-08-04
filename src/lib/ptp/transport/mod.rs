#[cfg(feature = "libusb")]
pub mod libusb;
#[cfg(all(windows, feature = "wpd"))]
pub mod wpd;

use std::{fmt, str::FromStr};

use anyhow::bail;

/// A single PTP transaction endpoint.
///
/// Implementors are responsible for framing (or delegating framing to the OS),
/// transaction IDs, and mapping the device response code onto
/// [`crate::ptp::error::Error::Response`].
pub trait PtpTransport {
    /// Execute one PTP transaction. `data` is the data-out payload, if any.
    /// Returns the data-in payload (empty when the response has no data phase).
    fn transact(
        &mut self,
        code: u16,
        params: &[u32],
        data: Option<&[u8]>,
    ) -> anyhow::Result<Vec<u8>>;

    fn open_session(&mut self, session_id: u32) -> anyhow::Result<()>;

    fn close_session(&mut self, session_id: u32) -> anyhow::Result<()>;

    fn chunk_size(&self) -> usize;

    /// Hint the preferred chunk size, as declared by the camera definition.
    /// Transports that negotiate their own transfer buffer size treat this as a cap.
    fn set_chunk_size(&mut self, chunk_size: usize);

    /// Opaque, round-trippable device identifier, as accepted by `-d`.
    fn id(&self) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportKind {
    /// Prefer WPD on Windows, fall back to libusb.
    #[default]
    Auto,
    Wpd,
    Libusb,
}

impl TransportKind {
    /// Preference order for [`TransportKind::Auto`]: WPD first so stock-driver
    /// users work out of the box, libusb second so Zadig users keep working.
    pub const ORDER: [Self; 2] = [Self::Wpd, Self::Libusb];

    #[must_use]
    pub fn candidates(self) -> Vec<Self> {
        match self {
            Self::Auto => Self::ORDER.to_vec(),
            kind => vec![kind],
        }
    }
}

impl FromStr for TransportKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "wpd" => Ok(Self::Wpd),
            "libusb" => Ok(Self::Libusb),
            _ => bail!("Invalid transport: {s}, expected one of auto, wpd, libusb"),
        }
    }
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Auto => "auto",
            Self::Wpd => "wpd",
            Self::Libusb => "libusb",
        };
        f.write_str(s)
    }
}

/// A discovered, not-yet-opened device.
pub enum Device {
    #[cfg(feature = "libusb")]
    Libusb(libusb::LibusbDevice),
    #[cfg(all(windows, feature = "wpd"))]
    Wpd(wpd::WpdDevice),
}

impl Device {
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            #[cfg(feature = "libusb")]
            Self::Libusb(d) => d.id(),
            #[cfg(all(windows, feature = "wpd"))]
            Self::Wpd(d) => d.id(),
        }
    }

    #[must_use]
    pub const fn vendor(&self) -> u16 {
        match self {
            #[cfg(feature = "libusb")]
            Self::Libusb(d) => d.vendor,
            #[cfg(all(windows, feature = "wpd"))]
            Self::Wpd(d) => d.vendor,
        }
    }

    #[must_use]
    pub const fn product(&self) -> u16 {
        match self {
            #[cfg(feature = "libusb")]
            Self::Libusb(d) => d.product,
            #[cfg(all(windows, feature = "wpd"))]
            Self::Wpd(d) => d.product,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> TransportKind {
        match self {
            #[cfg(feature = "libusb")]
            Self::Libusb(_) => TransportKind::Libusb,
            #[cfg(all(windows, feature = "wpd"))]
            Self::Wpd(_) => TransportKind::Wpd,
        }
    }

    pub fn open(&self) -> anyhow::Result<Box<dyn PtpTransport>> {
        match self {
            #[cfg(feature = "libusb")]
            Self::Libusb(d) => Ok(Box::new(d.open()?)),
            #[cfg(all(windows, feature = "wpd"))]
            Self::Wpd(d) => Ok(Box::new(d.open()?)),
        }
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("transport", &self.kind().to_string())
            .field("id", &self.id())
            .field("vendor", &format_args!("0x{:04x}", self.vendor()))
            .field("product", &format_args!("0x{:04x}", self.product()))
            .finish()
    }
}

/// Enumerate every device visible to `kind`.
///
/// [`TransportKind::Auto`] returns the union; callers that want the
/// WPD-then-libusb preference order should iterate [`TransportKind::ORDER`].
pub fn enumerate(kind: TransportKind) -> anyhow::Result<Vec<Device>> {
    match kind {
        TransportKind::Wpd => enumerate_wpd(),
        TransportKind::Libusb => enumerate_libusb(),
        TransportKind::Auto => {
            let mut devices = enumerate_wpd().unwrap_or_default();
            devices.extend(enumerate_libusb().unwrap_or_default());
            Ok(devices)
        }
    }
}

#[allow(clippy::unnecessary_wraps)]
fn enumerate_wpd() -> anyhow::Result<Vec<Device>> {
    #[cfg(all(windows, feature = "wpd"))]
    {
        Ok(wpd::enumerate()?.into_iter().map(Device::Wpd).collect())
    }
    #[cfg(not(all(windows, feature = "wpd")))]
    {
        Ok(Vec::new())
    }
}

#[allow(clippy::unnecessary_wraps)]
fn enumerate_libusb() -> anyhow::Result<Vec<Device>> {
    #[cfg(feature = "libusb")]
    {
        Ok(libusb::enumerate()?
            .into_iter()
            .map(Device::Libusb)
            .collect())
    }
    #[cfg(not(feature = "libusb"))]
    {
        Ok(Vec::new())
    }
}
