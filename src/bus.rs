//! An implementation of a `UsbBus` which provides interaction
//! methods for the send and receiving data to/from the device
//! from USB Host perspective.
//!
//! This implementation is not complete and probably buggy.
//!
use std::{cell::RefCell, cmp::min, rc::Rc};

use usb_device::bus::PollResult;
use usb_device::endpoint::{EndpointAddress, EndpointType};
use usb_device::{Result as UsbDeviceResult, UsbDirection, UsbError};

const NUM_ENDPOINTS: usize = 8;

/// Holds a simulated Endpoint status which allows bi-directional
/// communication via 1024 byte buffers.
struct EndpointImpl {
    ep_type: Option<EndpointType>,
    stall: bool,
    read_len: usize,
    read: [u8; 1024],
    read_ready: bool,
    write_len: usize,
    write: [u8; 1024],
    write_done: bool,
    setup: bool,
    max_size: usize,
}

impl EndpointImpl {
    fn new() -> Self {
        EndpointImpl {
            ep_type: None,
            stall: false,
            read_len: 0,
            read: [0; 1024],
            read_ready: false,
            write_len: 0,
            write: [0; 1024],
            write_done: false,
            setup: false,
            max_size: 0,
        }
    }

    /// Sets data that will be read by usb-device from the Endpoint
    fn set_read(&mut self, data: &[u8], setup: bool) -> usize {
        self.read_len = data.len();
        if self.read_len > 0 {
            self.read[..self.read_len].clone_from_slice(data);
            self.setup = setup;
            self.read_ready = true;
        }
        self.read_len
    }

    fn append_read(&mut self, data: &[u8]) -> usize {
        let len = data.len();

        if len > 0 {
            self.read[self.read_len..self.read_len + len].clone_from_slice(data);
            self.read_ready = true;
            self.read_len += len;
        }
        len
    }

    /// Returns data that was written by usb-device to the Endpoint
    fn get_write(&mut self, data: &mut [u8]) -> usize {
        let res = self.write_len;
        dbg!("g", self.write_len);
        self.write_len = 0;
        data[..res].clone_from_slice(&self.write[..res]);
        self.write_done = true;
        res
    }
}

/// Holds internal data like endpoints and provides
/// methods to access endpoint buffers like from
/// the "Host" side.
pub(crate) struct UsbBusImpl {
    ep_i: [RefCell<EndpointImpl>; NUM_ENDPOINTS],
    ep_o: [RefCell<EndpointImpl>; NUM_ENDPOINTS],
}

impl UsbBusImpl {
    pub(crate) fn new() -> Self {
        Self {
            ep_i: [
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
            ],
            ep_o: [
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
                RefCell::new(EndpointImpl::new()),
            ],
        }
    }

    fn epidx(&self, ep_addr: EndpointAddress) -> &RefCell<EndpointImpl> {
        match ep_addr.direction() {
            UsbDirection::In => self.ep_i.get(ep_addr.index()).unwrap(),
            UsbDirection::Out => self.ep_o.get(ep_addr.index()).unwrap(),
        }
    }

    pub(crate) fn get_write(&self, ep_addr: EndpointAddress, data: &mut [u8]) -> usize {
        let mut ep = self.epidx(ep_addr).borrow_mut();
        ep.get_write(data)
    }

    pub(crate) fn set_read(&self, ep_addr: EndpointAddress, data: &[u8], setup: bool) -> usize {
        let mut ep = self.epidx(ep_addr).borrow_mut();
        if setup && ep_addr.index() == 0 && ep_addr.direction() == UsbDirection::Out {
            // setup packet on EP0OUT removes stall condition
            ep.stall = false;
            let mut ep0in = self.ep_i.get(0).unwrap().borrow_mut();
            ep0in.stall = false;
        }
        ep.set_read(data, setup)
    }

    pub(crate) fn append_read(&self, ep_addr: EndpointAddress, data: &[u8]) -> usize {
        let mut ep = self.epidx(ep_addr).borrow_mut();
        ep.append_read(data)
    }

    pub(crate) fn ep_max_size(&self, ep_addr: EndpointAddress) -> usize {
        let ep = self.epidx(ep_addr).borrow();
        ep.max_size
    }

    pub(crate) fn ep_is_empty(&self, ep_addr: EndpointAddress) -> bool {
        let ep = self.epidx(ep_addr).borrow();
        match ep_addr.direction() {
            UsbDirection::In => ep.write_done,
            UsbDirection::Out => ep.read_ready,
        }
    }

    pub(crate) fn ep_data_len(&self, ep_addr: EndpointAddress) -> usize {
        let ep = self.epidx(ep_addr).borrow();
        match ep_addr.direction() {
            UsbDirection::In => ep.write_len,
            UsbDirection::Out => ep.read_len,
        }
    }

    pub(crate) fn stalled(&self, index: usize) -> bool {
        let addr_in = EndpointAddress::from_parts(index, UsbDirection::In);
        let addr_out = EndpointAddress::from_parts(index, UsbDirection::Out);
        {
            let ep = self.epidx(addr_in).borrow();
            if ep.stall {
                return true;
            }
        }
        {
            let ep = self.epidx(addr_out).borrow();
            if ep.stall {
                return true;
            }
        }
        false
    }
}

/// Implements `usb-device` UsbBus on top
/// of `UsbBusImpl`.
///
/// Not thread-safe.
pub struct EmulatedUsbBus {
    usb_address: RefCell<u8>,
    bus: Rc<RefCell<UsbBusImpl>>,
}

unsafe impl Sync for EmulatedUsbBus {}

impl EmulatedUsbBus {
    pub(crate) fn new(bus: &Rc<RefCell<UsbBusImpl>>) -> Self {
        Self {
            usb_address: RefCell::new(0),
            bus: bus.clone(),
        }
    }

    fn bus_ref(&self) -> &RefCell<UsbBusImpl> {
        self.bus.as_ref()
    }

    /// Returns USB Address assigned to Device
    /// by the Host.
    pub fn get_address(&self) -> u8 {
        *self.usb_address.borrow()
    }
}

impl usb_device::bus::UsbBus for EmulatedUsbBus {
    fn alloc_ep(
        &mut self,
        ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        ep_type: EndpointType,
        max_packet_size: u16,
        _interval: u8,
    ) -> UsbDeviceResult<EndpointAddress> {
        for index in ep_addr
            .map(|a| a.index()..a.index() + 1)
            .unwrap_or(1..NUM_ENDPOINTS)
        {
            let found_addr = EndpointAddress::from_parts(index, ep_dir);
            let io = self.bus_ref().borrow();
            let mut ep = io.epidx(found_addr).borrow_mut();

            match ep.ep_type {
                None => {
                    ep.ep_type = Some(ep_type);
                }
                Some(t) if t != ep_type => {
                    continue;
                }
                _ => {}
            };

            ep.stall = false;
            ep.max_size = max_packet_size as usize;

            return Ok(found_addr);
        }

        Err(match ep_addr {
            Some(_) => UsbError::InvalidEndpoint,
            None => UsbError::EndpointOverflow,
        })
    }

    fn enable(&mut self) {}

    fn force_reset(&self) -> UsbDeviceResult<()> {
        Err(UsbError::Unsupported)
    }

    fn poll(&self) -> PollResult {
        let mut mask_in_complete = 0;
        let mut mask_ep_out = 0;
        let mut mask_ep_setup = 0;

        for index in 0..NUM_ENDPOINTS {
            let addrin = EndpointAddress::from_parts(index, UsbDirection::In);
            let addrout = EndpointAddress::from_parts(index, UsbDirection::Out);
            let bit = 1 << index;

            let io = self.bus_ref().borrow();
            let ep_out = io.epidx(addrout).borrow();
            let mut ep_in = io.epidx(addrin).borrow_mut();

            if ep_in.write_done {
                mask_in_complete |= bit;
            }
            if ep_out.read_ready | ep_in.read_ready {
                mask_ep_out |= bit;
            }
            if ep_out.setup {
                mask_ep_setup |= bit;
            }

            ep_in.write_done = false;
        }

        // dbg!("WER", mask_in_complete, mask_ep_out, mask_ep_setup);
        if mask_in_complete != 0 || mask_ep_out != 0 || mask_ep_setup != 0 {
            PollResult::Data {
                ep_in_complete: mask_in_complete,
                ep_out: mask_ep_out,
                ep_setup: mask_ep_setup,
            }
        } else {
            PollResult::None
        }
    }

    fn read(&self, ep_addr: EndpointAddress, buf: &mut [u8]) -> UsbDeviceResult<usize> {
        let io = self.bus_ref().borrow();
        let mut ep = io.epidx(ep_addr).borrow_mut();
        let len = min(buf.len(), min(ep.read_len, ep.max_size));

        dbg!("read len from", buf.len(), len, ep_addr);

        if len == 0 {
            return Err(UsbError::WouldBlock);
        }

        buf[..len].clone_from_slice(&ep.read[..len]);

        ep.read_len -= len;
        ep.read.copy_within(len.., 0);

        if ep.read_len == 0 {
            ep.setup = false;
        }

        ep.read_ready = ep.read_len > 0;

        Ok(len)
    }

    fn reset(&self) {
        todo!()
    }

    fn resume(&self) {
        todo!()
    }

    fn suspend(&self) {
        todo!()
    }

    fn set_device_address(&self, addr: u8) {
        self.usb_address.replace(addr);
    }

    fn is_stalled(&self, ep_addr: EndpointAddress) -> bool {
        let io = self.bus_ref().borrow();
        let ep = io.epidx(ep_addr).borrow();
        ep.stall
    }

    fn set_stalled(&self, ep_addr: EndpointAddress, stalled: bool) {
        let io = self.bus_ref().borrow();
        let mut ep = io.epidx(ep_addr).borrow_mut();
        ep.stall = stalled;
    }

    fn write(&self, ep_addr: EndpointAddress, buf: &[u8]) -> UsbDeviceResult<usize> {
        let io = self.bus_ref().borrow();
        let mut ep = io.epidx(ep_addr).borrow_mut();
        let offset = ep.write_len;
        let mut len = 0;

        dbg!("write", buf.len());

        if buf.len() > ep.max_size {
            return Err(UsbError::BufferOverflow);
        }

        for (i, e) in ep.write[offset..].iter_mut().enumerate() {
            if i >= buf.len() {
                break;
            }
            *e = buf[i];
            len += 1;
        }

        dbg!("wrote", len);
        ep.write_len += len;
        ep.write_done = false;
        Ok(len)
    }
}
