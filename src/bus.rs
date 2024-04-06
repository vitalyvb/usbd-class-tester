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

/// Holds a simulated Endpoint status which allows bi-directional
/// communication via 1024 byte buffers.
struct EndpointImpl {
    alloc: bool,
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
            alloc: false,
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
    fn set_read(&mut self, data: &[u8], setup: bool) {
        self.read_len = data.len();
        if self.read_len > 0 {
            self.read[..self.read_len].clone_from_slice(data);
            self.setup = setup;
            self.read_ready = true;
        }
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
    ep_i: [RefCell<EndpointImpl>; 4],
    ep_o: [RefCell<EndpointImpl>; 4],
}

impl UsbBusImpl {
    pub(crate) fn new() -> Self {
        Self {
            ep_i: [
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

    pub(crate) fn set_read(&self, ep_addr: EndpointAddress, data: &[u8], setup: bool) {
        let mut ep = self.epidx(ep_addr).borrow_mut();
        if setup && ep_addr.index() == 0 && ep_addr.direction() == UsbDirection::Out {
            // setup packet on EP0OUT removes stall condition
            ep.stall = false;
            let mut ep0in = self.ep_i.get(0).unwrap().borrow_mut();
            ep0in.stall = false;
        }
        ep.set_read(data, setup)
    }

    pub(crate) fn stalled0(&self) -> bool {
        let in0 = EndpointAddress::from_parts(0, UsbDirection::In);
        let out0 = EndpointAddress::from_parts(0, UsbDirection::Out);
        {
            let ep = self.epidx(in0).borrow();
            if ep.stall {
                return true;
            }
        }
        {
            let ep = self.epidx(out0).borrow();
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
        _ep_dir: UsbDirection,
        ep_addr: Option<EndpointAddress>,
        _ep_type: EndpointType,
        max_packet_size: u16,
        _interval: u8,
    ) -> UsbDeviceResult<EndpointAddress> {
        if let Some(ea) = ep_addr {
            let io = self.bus_ref().borrow();
            let mut sep = io.epidx(ea).borrow_mut();

            if sep.alloc {
                return Err(UsbError::InvalidEndpoint);
            }

            sep.alloc = true;
            sep.stall = false;
            sep.max_size = max_packet_size as usize;

            Ok(ea)
        } else {
            // ep_addr is required, endpoint allocation is not implemented
            Err(UsbError::EndpointMemoryOverflow)
        }
    }

    fn enable(&mut self) {}

    fn force_reset(&self) -> UsbDeviceResult<()> {
        Err(UsbError::Unsupported)
    }

    fn poll(&self) -> PollResult {
        let in0 = EndpointAddress::from_parts(0, UsbDirection::In);
        let out0 = EndpointAddress::from_parts(0, UsbDirection::Out);

        let io = self.bus_ref().borrow();
        let ep0out = io.epidx(out0).borrow();
        let mut ep0in = io.epidx(in0).borrow_mut();

        let ep0_write_done = ep0in.write_done;
        let ep0_can_read = ep0out.read_ready | ep0in.read_ready;
        let ep0_setup = ep0out.setup;

        ep0in.write_done = false;
        // dbg!(ep0out.read_ready , ep0in.read_ready);

        dbg!(ep0_write_done, ep0_can_read, ep0_setup);

        if ep0_write_done || ep0_can_read || ep0_setup {
            PollResult::Data {
                ep_in_complete: if ep0_write_done { 1 } else { 0 },
                ep_out: if ep0_can_read { 1 } else { 0 },
                ep_setup: if ep0_setup { 1 } else { 0 },
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
