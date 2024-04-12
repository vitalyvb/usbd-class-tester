//! A collection of USB-related data-handling stuff.
//!
//! Must not use `usb-device` implementation to
//! be able to test anything.
//!

/// `CtrRequestType` holds bmRequestType of SETUP
/// packet.
#[must_use]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct CtrRequestType {
    direction: u8,
    rtype: u8,
    recipient: u8,
}

impl CtrRequestType {
    /// Create new `CtrRequestType` with Host-to-device
    /// direction (0b00000000).
    pub fn to_device() -> Self {
        CtrRequestType {
            direction: 0,
            rtype: 0,
            recipient: 0,
        }
    }

    /// Create new `CtrRequestType` with Device-to-host
    /// direction  (0b10000000).
    pub fn to_host() -> Self {
        CtrRequestType {
            direction: 1,
            rtype: 0,
            recipient: 0,
        }
    }

    /// Copy and set Type to Standard (0bx00xxxxx)
    pub fn standard(self) -> Self {
        CtrRequestType { rtype: 0, ..self }
    }

    /// Copy and set Type to Class (0bx01xxxxx)
    pub fn class(self) -> Self {
        CtrRequestType { rtype: 1, ..self }
    }

    /// Copy and set Type to Vendor (0bx10xxxxx)
    pub fn vendor(self) -> Self {
        CtrRequestType { rtype: 2, ..self }
    }

    /// Copy and set Recipient to Device (0bxxx00000)
    pub fn device(self) -> Self {
        CtrRequestType {
            recipient: 0,
            ..self
        }
    }

    /// Copy and set Recipient to Interface (0bxxx00001)
    pub fn interface(self) -> Self {
        CtrRequestType {
            recipient: 1,
            ..self
        }
    }

    /// Copy and set Recipient to Endpoint (0bxxx00010)
    pub fn endpoint(self) -> Self {
        CtrRequestType {
            recipient: 2,
            ..self
        }
    }

    /// Copy and set Recipient to Other (0bxxx00011)
    pub fn other(self) -> Self {
        CtrRequestType {
            recipient: 3,
            ..self
        }
    }
}

impl From<CtrRequestType> for u8 {
    fn from(val: CtrRequestType) -> Self {
        val.direction << 7 | val.rtype << 5 | val.recipient
    }
}

impl From<u8> for CtrRequestType {
    fn from(value: u8) -> Self {
        CtrRequestType {
            direction: value >> 7,
            rtype: (value >> 5) & 0x3,
            recipient: value & 0x1f,
        }
    }
}

/// `SetupPacket` structure holds SETUP packet data for
/// all Control transfers.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SetupPacket {
    bm_request_type: CtrRequestType,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
}

impl SetupPacket {
    /// Create new SetupPacket
    pub fn new(reqt: CtrRequestType, req: u8, value: u16, index: u16, length: u16) -> Self {
        SetupPacket {
            bm_request_type: reqt,
            b_request: req,
            w_value: value,
            w_index: index,
            w_length: length,
        }
    }
}

impl From<SetupPacket> for [u8; 8] {
    fn from(val: SetupPacket) -> Self {
        [
            val.bm_request_type.into(),
            val.b_request,
            (val.w_value & 0xff) as u8,
            (val.w_value >> 8) as u8,
            (val.w_index & 0xff) as u8,
            (val.w_index >> 8) as u8,
            (val.w_length & 0xff) as u8,
            (val.w_length >> 8) as u8,
        ]
    }
}
