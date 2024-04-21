#![allow(clippy::test_attr_in_doctest)]
#![warn(missing_docs)]
//!
//! A library for running tests of `usb-device` classes on
//! developer's system natively.
//!
//! ## About
//!
//! Testing is difficult, and if it's even more difficult
//! when it involves a dedicated hardware and doing
//! the test manually. Often a lot of stuff needs to be
//! re-tested even after small code changes.
//!
//! This library aims to help testing the implementation of
//! protocols in USB devices which are based on `usb-device`
//! crate by providing a means of simulating Host's accesses
//! to the device.
//!
//! Initial implementation was done for tests in `usbd-dfu`
//! crate. This library is based on that idea, but extends
//! it a lot. For example it adds a set of convenience
//! functions for Control transfers, while originally this
//! was done via plain `u8` arrays only.
//!
//! ### Supported operations
//!
//! * IN and OUT EP0 control transfers
//! * Transfers on other endpoints (e.g. Interrupt)
//!
//! ### Not supported operations
//!
//! Almost everything else, including but not limited to:
//!
//! * Reset
//! * Suspend and Resume
//! * Bulk transfers
//! * Iso transfers
//! * ...
//!
//! ## License
//!
//! This project is licensed under [MIT License](https://opensource.org/licenses/MIT)
//! ([LICENSE](https://github.com/vitalyvb/usbd-class-tester/blob/main/LICENSE)).
//!
//! ### Contribution
//!
//! Unless you explicitly state otherwise, any contribution intentionally
//! submitted for inclusion in the work by you shall be licensed as above,
//! without any additional terms or conditions.
//!
//! ## Example
//!
//! The example defines an empty `UsbClass` implementation for `TestUsbClass`.
//! Normally this would also include things like endpoint allocations,
//! device-specific descriptor generation, and the code handling everything.
//! This is not in the scope of this example.
//!
//! A minimal `TestCtx` creates `TestUsbClass` that will be passed to
//! a test case. In general, `TestCtx` allows some degree of environment
//! customization, like choosing EP0 transfer size, or redefining how
//! `UsbDevice` is created.
//!
//! Check crate tests directory for more examples.
//!
//! Also see the documentation for `usb-device`.
//!
//! ```
//! use usb_device::class_prelude::*;
//! use usbd_class_tester::prelude::*;
//!
//! // `UsbClass` under the test.
//! pub struct TestUsbClass {}
//! impl<B: UsbBus> UsbClass<B> for TestUsbClass {}
//!
//! // Context to create a testable instance of `TestUsbClass`
//! struct TestCtx {}
//! impl UsbDeviceCtx for TestCtx {
//!     type C<'c> = TestUsbClass;
//!     fn create_class<'a>(
//!         &mut self,
//!         alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
//!     ) -> AnyResult<TestUsbClass> {
//!         Ok(TestUsbClass {})
//!     }
//! }
//!
//! #[test]
//! fn test_interface_get_status() {
//!     TestCtx {}
//!         .with_usb(|mut cls, mut dev| {
//!             let st = dev.interface_get_status(&mut cls, 0).expect("status");
//!             assert_eq!(st, 0);
//!         })
//!         .expect("with_usb");
//! }
//! ```
//!
//! USB debug logging can be enabled, for example, by running tests with:
//! `$ RUST_LOG=trace cargo test -- --nocapture`
//!

use log::{debug, info, warn};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::{cell::RefCell, rc::Rc};

use usb_device::bus::UsbBusAllocator;
use usb_device::class::UsbClass;
use usb_device::device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid};
use usb_device::endpoint::EndpointAddress;
use usb_device::prelude::BuilderError;
use usb_device::UsbDirection;

mod bus;
use bus::*;

mod usbdata;
use usbdata::*;

/// Prelude
pub mod prelude {
    pub use crate::bus::EmulatedUsbBus;
    pub use crate::usbdata::{CtrRequestType, SetupPacket};
    pub use crate::{AnyResult, AnyUsbError, Device, HookAction, HookWhen, UsbDeviceCtx};
}

const DEFAULT_EP0_SIZE: u8 = 8;
const DEFAULT_ADDRESS: u8 = 5;

/// Possible errors or other abnormal
/// conditions.
#[derive(Debug, PartialEq)]
pub enum AnyUsbError {
    /// EP Stalled after Setup packet. Not
    /// necessarily an error, the Device
    /// rejected EP transaction.
    /// Next request should clear Stall for EP0.
    EP0Stalled,
    /// EP0 buffer is not empty after Setup
    /// packet was consumed.
    EP0NotEmptyAfterSetup,
    /// Can't get how many bytes were written.
    /// Usually, this is some internal error.
    EPWriteError,
    /// Can't get how many bytes were read.
    /// Usually, this is some internal error.
    EPReadError,
    /// EP Stalled. Not necessarily an error,
    /// the Device rejected EP transaction.
    EPStalled,
    /// Error while reading from the endpoint.
    /// No data or data limit reached.
    /// Usually, this is some internal error.
    EPReadFailed,
    /// Bad reply length for GET_STATUS control request.
    /// Length should be 2.
    /// Usually, this is some internal error.
    EP0BadGetStatusSize,
    /// Bad reply length for GET_CONFIGURATION control request.
    /// Length should be 1.
    /// Usually, this is some internal error.
    EP0BadGetConfigSize,
    /// Failed to convert one data representation
    /// to another, e.g. with TryInto.
    /// Usually, this is some internal error.
    DataConversion,
    /// SET_ADDRESS didn't work during Device setup.
    /// Usually, this is some internal error.
    SetAddressFailed,
    /// Descriptor length is larger than the size
    /// of data returned.
    InvalidDescriptorLength,
    /// Unexpected Descriptor type.
    InvalidDescriptorType,
    /// String Descriptor length is odd.
    InvalidStringLength,
    /// Wrapper for `BuilderError` of `usb-device`
    /// when `UsbDeviceBuilder` fails.
    UsbDeviceBuilder(BuilderError),
    /// User-defined meaning.
    /// Enum value is passed through, not used by the library.
    UserDefined1,
    /// User-defined meaning
    /// Enum value is passed through, not used by the library.
    UserDefined2,
    /// User-defined meaning
    /// Enum value is passed through, not used by the library.
    UserDefinedU64(u64),
    /// User-defined meaning
    /// Enum value is passed through, not used by the library.
    UserDefinedString(String),
}

/// Specifies why `Device::hook()` was called.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HookWhen {
    /// After `poll()` from `with_usb()` after initialization
    /// and before device setup.
    InitIdle,
    /// After `poll()` once Setup packet is sent during the transaction.
    AfterSetup(EndpointAddress),
    /// After `poll()` once some portion of data is sent to
    /// the device.
    DataOut(EndpointAddress),
    /// After `poll()` once some portion of data is received from
    /// the device.
    DataIn(EndpointAddress),
    /// After a manual `poll()` from `with_usb()`'s `case`.
    ManualPoll,
}

/// Specifies what `Device::hook()`'s caller should
/// do.
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub enum HookAction {
    /// Proceed normally
    #[default]
    Default,
    /// Force polling again.
    ForcePoll,
    /// Do not call `poll` again even if would
    /// call it normally and act as if `poll`
    /// returned `false`.
    Stop,
}

/// Holds results for endpoint read/write operations
#[derive(Debug, Default)]
pub struct RWRes {
    /// If there was a read operation returns number of data bytes
    /// that were read.
    pub read: Option<usize>,
    /// If there was a write operation returns number of data bytes
    /// that were written.
    /// Setup packet is not included.
    pub wrote: Option<usize>,
}

impl RWRes {
    fn new(read: Option<usize>, wrote: Option<usize>) -> Self {
        Self { read, wrote }
    }
}

/// Result for crate operations.
pub type AnyResult<T> = core::result::Result<T, AnyUsbError>;

/// A context for the test, provides some
/// configuration values, initialization,
/// and some customization.
pub trait UsbDeviceCtx: Sized {
    /// Class under the test.
    /// # Examples
    /// ```ignore
    /// type C<'c> = SimpleUsbClass;
    /// type C<'c> = ComplexUsbClass<'c, EmulatedUsbBus>;
    /// ```

    type C<'c>: UsbClass<EmulatedUsbBus> + 'c;

    /// EP0 size used by `build_usb_device()` when creating
    /// `UsbDevice`.
    ///
    /// Incorrect values should cause `UsbDeviceBuilder` to
    /// fail.
    const EP0_SIZE: u8 = DEFAULT_EP0_SIZE;

    /// Address the Device gets assigned.
    ///
    /// A properly configured Device should get
    /// a non-zero address.
    const ADDRESS: u8 = DEFAULT_ADDRESS;

    /// Create `UsbClass` object.
    /// # Example
    /// ```
    /// # use usb_device::class_prelude::*;
    /// # use usbd_class_tester::prelude::*;
    /// # struct TestUsbClass {}
    /// # impl TestUsbClass {
    /// #     pub fn new<B: UsbBus>(_alloc: &UsbBusAllocator<B>) -> Self {Self {}}
    /// # }
    /// # trait DOC<B: UsbBus> {
    /// fn create_class<'a>(
    ///     &mut self,
    ///     alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    /// ) -> AnyResult<TestUsbClass> {
    ///     Ok(TestUsbClass::new(&alloc))
    /// }
    /// # }
    /// ```
    fn create_class<'a>(
        &mut self,
        alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<Self::C<'a>>;

    /// Optional. Called after each `usb-device` `poll()`.
    ///
    /// Default implementation does nothing.
    fn hook(&mut self, cls: &mut Self::C<'_>, when: HookWhen) -> HookAction {
        let _ = cls;
        let _ = when;
        HookAction::Default
    }

    /// Optional. Called by `with_usb` every time.
    ///
    /// Default implementation initializes `env_logger` logging suitable
    /// for tests with `debug` level if `initlog` feature is enabled.
    ///
    /// `trace` logging level can be enabled via environment variable:
    /// `RUST_LOG=trace cargo test -- --nocapture`.
    fn initialize(&mut self) {
        #[cfg(feature = "initlog")]
        let _ =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
                .is_test(true)
                .format_target(false)
                .format_timestamp(None)
                .try_init();
    }

    /// Optional. If returns `true`, `Device::setup()` is not
    /// called to initialize and enumerate device in
    /// `with_usb()`.
    ///
    /// `with_usb()`'s `case` will be called with a
    /// non-configured/non-enumerated device.
    ///
    /// Default implementation always returns `false`.
    fn skip_setup(&mut self) -> bool {
        false
    }

    /// Optional. Implementation overrides the creation of `UsbDevice`
    /// if the default implementation needs changing.
    /// # Example
    /// ```
    /// # use usb_device::prelude::*;
    /// # use usb_device::class_prelude::*;
    /// # use usbd_class_tester::AnyResult;
    /// # trait DOC<B: UsbBus> {
    /// fn build_usb_device<'a>(&mut self, alloc: &'a UsbBusAllocator<B>) -> AnyResult<UsbDevice<'a, B>> {
    ///     let usb_dev = UsbDeviceBuilder::new(alloc, UsbVidPid(0, 0))
    ///      // .strings()
    ///      // .max_packet_size_0()
    ///      // ...
    ///         .build();
    ///     Ok(usb_dev)
    /// }
    /// # }
    /// ````
    fn build_usb_device<'a>(
        &mut self,
        alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<UsbDevice<'a, EmulatedUsbBus>> {
        let usb_dev = UsbDeviceBuilder::new(alloc, UsbVidPid(0x1234, 0x5678))
            .strings(&[StringDescriptors::default()
                .manufacturer("TestManufacturer")
                .product("TestProduct")
                .serial_number("TestSerial")])
            .map_err(AnyUsbError::UsbDeviceBuilder)?
            .device_release(0x0200)
            .self_powered(true)
            .max_power(250)
            .map_err(AnyUsbError::UsbDeviceBuilder)?
            .max_packet_size_0(Self::EP0_SIZE)
            .map_err(AnyUsbError::UsbDeviceBuilder)?
            .build();

        Ok(usb_dev)
    }

    /// Initialize USB device Class `C` according to the provided
    /// context `X` and run `case()` on it.
    ///
    /// `case` will not be called if `with_usb` encounters a
    /// problem during the setup, in this case `with_usb` returns
    /// an error.
    ///
    /// # Example
    /// ```
    /// use usb_device::class_prelude::*;
    /// use usbd_class_tester::prelude::*;
    ///
    /// pub struct TestUsbClass {}
    /// impl<B: UsbBus> UsbClass<B> for TestUsbClass {}
    ///
    /// struct TestCtx {}
    /// impl TestCtx {}
    ///
    /// impl UsbDeviceCtx for TestCtx {
    ///     type C<'c> = TestUsbClass;
    ///     fn create_class<'a>(
    ///         &mut self,
    ///         alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ///     ) -> AnyResult<TestUsbClass> {
    ///         Ok(TestUsbClass {})
    ///     }
    /// }
    ///
    /// #[test]
    /// fn test_interface_get_status() {
    ///     with_usb(TestCtx {}, |mut cls, mut dev| {
    ///         let st = dev.interface_get_status(&mut cls, 0).expect("status");
    ///         assert_eq!(st, 0);
    ///     })
    ///     .expect("with_usb");
    /// }
    /// ```
    ///
    fn with_usb(
        mut self,
        case: for<'a> fn(cls: Self::C<'a>, dev: Device<'a, Self::C<'a>, Self>),
    ) -> AnyResult<()> {
        self.initialize();

        warn!("#### with_usb start");

        let stio: UsbBusImpl = UsbBusImpl::new();
        let io = Rc::new(RefCell::new(stio));
        let bus = EmulatedUsbBus::new(&io);

        let alloc: usb_device::bus::UsbBusAllocator<EmulatedUsbBus> = UsbBusAllocator::new(bus);

        let mut cls = self.create_class(&alloc)?;

        let usb_dev = self.build_usb_device(&alloc)?;

        let skip_setup = self.skip_setup();

        let mut dev = Device::new(io.as_ref(), self, usb_dev);

        dev.do_poll(&mut cls, HookWhen::InitIdle);

        if !skip_setup {
            warn!("#### with_usb device setup");
            dev.setup(&mut cls)?;
        }

        // run test
        warn!("#### with_usb case");
        case(cls, dev);

        Ok(())
    }
}

/// Represents Host's view of the Device via
/// USB bus.
pub struct Device<'a, C, X>
where
    C: UsbClass<EmulatedUsbBus>,
    X: UsbDeviceCtx<C<'a> = C>,
{
    ctx: X,
    usb: &'a RefCell<UsbBusImpl>,
    dev: UsbDevice<'a, EmulatedUsbBus>,
    _cls: PhantomData<C>,
}

impl<'a, C, X> Device<'a, C, X>
where
    C: UsbClass<EmulatedUsbBus>,
    X: UsbDeviceCtx<C<'a> = C>,
{
    fn new(usb: &'a RefCell<UsbBusImpl>, ctx: X, dev: UsbDevice<'a, EmulatedUsbBus>) -> Self {
        Device {
            usb,
            ctx,
            dev,
            _cls: PhantomData,
        }
    }

    /// Provides direct access to `EmulatedUsbBus`
    pub fn usb_dev(&mut self) -> &mut UsbDevice<'a, EmulatedUsbBus> {
        &mut self.dev
    }

    fn do_poll(&mut self, d: &mut C, when: HookWhen) -> bool {
        let mut res;
        loop {
            res = self.dev.poll(&mut [d]);
            match self.ctx.hook(d, when) {
                HookAction::Default => return res,
                HookAction::ForcePoll => continue,
                HookAction::Stop => return false,
            }
        }
    }

    /// Call `usb-device` poll().
    ///
    /// Most `Device` operations call poll() automatically
    /// and this is usually enough for smaller transfers.
    ///
    /// Must be called, for example, to gradually process
    /// emulated endpoint's buffer if `UsbClass`` blocks and
    /// is unable to process data until some action is
    /// pefromed on it.
    pub fn poll(&mut self, d: &mut C) -> bool {
        self.do_poll(d, HookWhen::ManualPoll)
    }

    /// Perform EP0 Control transfer. `setup` is `SetupPacket`.
    /// If transfer is Host-to-device and
    /// `data` is `Some`, then it's sent after the Setup packet
    /// and Device can receive it as a payload. For Device-to-host
    /// transfers `data` should be `None` and `out` must have
    /// enough space to store the response.
    pub fn ep0(
        &mut self,
        d: &mut C,
        setup: SetupPacket,
        data: Option<&[u8]>,
        out: &mut [u8],
    ) -> core::result::Result<RWRes, AnyUsbError> {
        let setup_bytes: [u8; 8] = setup.into();
        self.ep_raw(d, 0, Some(&setup_bytes), data, out)
    }

    /// Perform Endpoint Device-to-host data transfer
    /// on a given endpoint index `ep_index` of a
    /// maximum size `length`.
    ///
    /// Returns a Vec[u8] with data.
    pub fn ep_read(
        &mut self,
        cls: &mut C,
        ep_index: usize,
        length: u16,
    ) -> core::result::Result<Vec<u8>, AnyUsbError> {
        let mut buf: Vec<u8> = vec![0; length as usize];

        let len = self.ep_raw(cls, ep_index, None, None, buf.as_mut_slice())?;

        if let Some(len) = len.read {
            buf.truncate(len);
            Ok(buf)
        } else {
            Err(AnyUsbError::EPReadError)
        }
    }

    /// Perform Endpoint Host-to-device data transfer
    /// on a given endpoint index `ep_index` and
    /// with `data`.
    ///
    /// Returns number of bytes that was loaded into
    /// Endpoint buffer.
    pub fn ep_write(
        &mut self,
        cls: &mut C,
        ep_index: usize,
        data: &[u8],
    ) -> core::result::Result<usize, AnyUsbError> {
        let len = self.ep_raw(cls, ep_index, None, Some(data), &mut [])?;
        len.wrote.ok_or(AnyUsbError::EPWriteError)
    }

    /// Perform raw EP0 Control transfer. `setup_bytes` is a
    /// 8-byte Setup packet. If transfer is Host-to-device and
    /// `data` is `Some`, then it's sent after the Setup packet
    /// and Device can receive it as a payload. For Device-to-host
    /// transfers `data` should be `None` and `out` must have
    /// enough space to store the response.
    pub fn ep_raw(
        &mut self,
        d: &mut C,
        ep_index: usize,
        setup_bytes: Option<&[u8]>,
        data: Option<&[u8]>,
        out: &mut [u8],
    ) -> core::result::Result<RWRes, AnyUsbError> {
        let mut sent = None;
        let out0 = EndpointAddress::from_parts(ep_index, UsbDirection::Out);
        let in0 = EndpointAddress::from_parts(ep_index, UsbDirection::In);

        info!("#### EP {} transaction", ep_index);

        if let Some(setup_bytes) = setup_bytes {
            self.usb.borrow().set_read(out0, setup_bytes, true);
            self.do_poll(d, HookWhen::AfterSetup(out0));
            if self.usb.borrow().stalled(ep_index) {
                return Err(AnyUsbError::EP0Stalled);
            }
            if self.usb.borrow().ep_data_len(out0) != 0 {
                return Err(AnyUsbError::EP0NotEmptyAfterSetup);
            }
        }

        if let Some(val) = data {
            sent = Some(self.usb.borrow().append_read(out0, val));
            for i in 1..129 {
                let before_bytes = self.usb.borrow().ep_data_len(out0);
                let res = self.do_poll(d, HookWhen::DataIn(out0));
                let after_bytes = self.usb.borrow().ep_data_len(out0);

                if !res {
                    debug!("#### EP {} class has no data to consume", ep_index);
                    break;
                }
                if self.usb.borrow().ep_is_empty(out0) {
                    debug!("#### EP {} consumed all data", ep_index);
                    break;
                }
                if before_bytes == after_bytes {
                    debug!(
                        "#### EP {} poll didn't consume any data, have {} bytes",
                        ep_index, after_bytes
                    );
                    break;
                }
                if i >= 128 {
                    return Err(AnyUsbError::EPReadFailed);
                }
            }
            if self.usb.borrow().stalled(ep_index) {
                return Err(AnyUsbError::EPStalled);
            }
        }

        let mut len = 0;
        let max_ep_size = self.usb.borrow().ep_max_size(in0);

        loop {
            let one = self.usb.borrow().get_write(in0, &mut out[len..]);
            self.do_poll(d, HookWhen::DataOut(in0));
            if self.usb.borrow().stalled(ep_index) {
                return Err(AnyUsbError::EPStalled);
            }

            len += one;
            if one < max_ep_size {
                // short read - last block
                break;
            }
        }

        Ok(RWRes::new(Some(len), sent))
    }

    /// Perform EP0 Control transfer.
    /// If transfer is Host-to-device and
    /// `data` is `Some`, then it's sent after the Setup packet
    /// and Device can receive it as a payload. For Device-to-host
    /// transfers `data` should be `None` and the response
    /// is returned in a result `Vec`.
    #[allow(clippy::too_many_arguments)]
    pub fn ep_io_control(
        &mut self,
        cls: &mut C,
        reqt: CtrRequestType,
        req: u8,
        value: u16,
        index: u16,
        length: u16,
        data: Option<&[u8]>,
    ) -> core::result::Result<Vec<u8>, AnyUsbError> {
        let mut buf: Vec<u8> = vec![0; length as usize];

        let setup = SetupPacket::new(reqt, req, value, index, length);

        let len = self.ep0(cls, setup, data, buf.as_mut_slice())?;

        if let Some(len) = len.read {
            buf.truncate(len);
            Ok(buf)
        } else {
            Err(AnyUsbError::EPReadError)
        }
    }

    /// Perform Device-to-host EP0 Control transfer.
    /// The response is returned in a result `Vec`.
    ///
    /// `reqt` is passed as is. It should be `to_host()`.
    pub fn control_read(
        &mut self,
        cls: &mut C,
        reqt: CtrRequestType,
        req: u8,
        value: u16,
        index: u16,
        length: u16,
    ) -> core::result::Result<Vec<u8>, AnyUsbError> {
        self.ep_io_control(cls, reqt, req, value, index, length, None)
    }

    /// Perform Host-to-device EP0 Control transfer.
    /// `data` is sent after the Setup packet
    /// and Device can receive it as a payload.
    /// The response is returned in a result `Vec`
    /// and normally it should be empty.
    ///
    /// `reqt` is passed as is. It should be `to_device()`.
    #[allow(clippy::too_many_arguments)]
    pub fn control_write(
        &mut self,
        cls: &mut C,
        reqt: CtrRequestType,
        req: u8,
        value: u16,
        index: u16,
        length: u16,
        data: &[u8],
    ) -> core::result::Result<Vec<u8>, AnyUsbError> {
        self.ep_io_control(cls, reqt, req, value, index, length, Some(data))
    }

    /// Standard Device Request: GET_STATUS (0x00)
    pub fn device_get_status(&mut self, cls: &mut C) -> core::result::Result<u16, AnyUsbError> {
        let data = self.control_read(cls, CtrRequestType::to_host(), 0, 0, 0, 2)?;
        if data.len() != 2 {
            return Err(AnyUsbError::EP0BadGetStatusSize);
        }

        let res = data.try_into().map_err(|_| AnyUsbError::DataConversion)?;
        Ok(u16::from_le_bytes(res))
    }

    /// Standard Device Request: CLEAR_FEATURE (0x01)
    pub fn device_clear_feature(
        &mut self,
        cls: &mut C,
        feature: u16,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(cls, CtrRequestType::to_device(), 1, feature, 0, 0, &[])
            .and(Ok(()))
    }

    /// Standard Device Request: SET_FEATURE (0x03)
    pub fn device_set_feature(
        &mut self,
        cls: &mut C,
        feature: u16,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(cls, CtrRequestType::to_device(), 3, feature, 0, 0, &[])
            .and(Ok(()))
    }

    /// Standard Device Request: SET_ADDRESS (0x05)
    pub fn device_set_address(
        &mut self,
        cls: &mut C,
        address: u8,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device(),
            5,
            address as u16,
            0,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Device Request: GET_DESCRIPTOR (0x06)
    pub fn device_get_descriptor(
        &mut self,
        cls: &mut C,
        dtype: u8,
        dindex: u8,
        lang_id: u16,
        length: u16,
    ) -> core::result::Result<Vec<u8>, AnyUsbError> {
        let typeindex: u16 = ((dtype as u16) << 8) | dindex as u16;
        self.control_read(
            cls,
            CtrRequestType::to_host(),
            6,
            typeindex,
            lang_id,
            length,
        )
    }

    /// Get String descriptor from the device and return
    /// unicode string.
    ///
    /// Standard Device Request: GET_DESCRIPTOR (0x06)
    pub fn device_get_string(
        &mut self,
        cls: &mut C,
        index: u8,
        lang_id: u16,
    ) -> core::result::Result<String, AnyUsbError> {
        let typeindex: u16 = (3u16 << 8) | index as u16;
        let descr =
            self.control_read(cls, CtrRequestType::to_host(), 6, typeindex, lang_id, 255)?;

        if descr.len() < 2 {
            return Err(AnyUsbError::InvalidDescriptorLength);
        }

        if descr[0] as usize != descr.len() {
            return Err(AnyUsbError::InvalidDescriptorLength);
        }

        if descr[1] != 3 {
            return Err(AnyUsbError::InvalidDescriptorType);
        }

        if descr[0] % 2 != 0 {
            return Err(AnyUsbError::InvalidStringLength);
        }

        let vu16: Vec<u16> = descr[2..]
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let res = String::from_utf16(&vu16).map_err(|_| AnyUsbError::DataConversion)?;

        Ok(res)
    }

    /// Standard Device Request: SET_DESCRIPTOR (0x07)
    pub fn device_set_descriptor(
        &mut self,
        cls: &mut C,
        dtype: u8,
        dindex: u8,
        lang_id: u16,
        length: u16,
        data: &[u8],
    ) -> core::result::Result<(), AnyUsbError> {
        let typeindex: u16 = ((dtype as u16) << 8) | dindex as u16;
        self.control_write(
            cls,
            CtrRequestType::to_device(),
            7,
            typeindex,
            lang_id,
            length,
            data,
        )
        .and(Ok(()))
    }

    /// Standard Device Request: GET_CONFIGURATION (0x08)
    pub fn device_get_configuration(
        &mut self,
        cls: &mut C,
    ) -> core::result::Result<u8, AnyUsbError> {
        let res = self.control_read(cls, CtrRequestType::to_host(), 8, 0, 0, 1)?;
        if res.len() != 1 {
            return Err(AnyUsbError::EP0BadGetConfigSize);
        }
        Ok(res[0])
    }

    /// Standard Device Request: SET_CONFIGURATION (0x09)
    pub fn device_set_configuration(
        &mut self,
        cls: &mut C,
        configuration: u8,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device(),
            9,
            configuration as u16,
            0,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Interface Request: GET_STATUS (0x00)
    pub fn interface_get_status(
        &mut self,
        cls: &mut C,
        interface: u8,
    ) -> core::result::Result<u16, AnyUsbError> {
        let data = self.control_read(
            cls,
            CtrRequestType::to_host().interface(),
            0,
            0,
            interface as u16,
            2,
        )?;
        if data.len() != 2 {
            return Err(AnyUsbError::EP0BadGetStatusSize);
        }

        let res = data.try_into().map_err(|_| AnyUsbError::DataConversion)?;
        Ok(u16::from_le_bytes(res))
    }

    /// Standard Interface Request: CLEAR_FEATURE (0x01)
    pub fn interface_clear_feature(
        &mut self,
        cls: &mut C,
        interface: u8,
        feature: u16,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device().interface(),
            1,
            feature,
            interface as u16,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Interface Request: SET_FEATURE (0x03)
    pub fn interface_set_feature(
        &mut self,
        cls: &mut C,
        interface: u8,
        feature: u16,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device().interface(),
            3,
            feature,
            interface as u16,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Interface Request: GET_INTERFACE (0x0a)
    pub fn interface_get_interface(
        &mut self,
        cls: &mut C,
    ) -> core::result::Result<u8, AnyUsbError> {
        let res = self.control_read(cls, CtrRequestType::to_host().interface(), 10, 0, 0, 1)?;
        if res.len() != 1 {
            return Err(AnyUsbError::EP0BadGetConfigSize);
        }
        Ok(res[0])
    }

    /// Standard Interface Request: SET_INTERFACE (0x0b)
    pub fn interface_set_interface(
        &mut self,
        cls: &mut C,
        interface: u8,
        alt_setting: u8,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device().interface(),
            11,
            alt_setting as u16,
            interface as u16,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Endpoint Request: GET_STATUS (0x00)
    pub fn endpoint_get_status(
        &mut self,
        cls: &mut C,
        endpoint: u8,
    ) -> core::result::Result<u16, AnyUsbError> {
        let data = self.control_read(
            cls,
            CtrRequestType::to_host().endpoint(),
            0,
            0,
            endpoint as u16,
            2,
        )?;
        if data.len() != 2 {
            return Err(AnyUsbError::EP0BadGetStatusSize);
        }

        let res = data.try_into().map_err(|_| AnyUsbError::DataConversion)?;
        Ok(u16::from_le_bytes(res))
    }

    /// Standard Endpoint Request: CLEAR_FEATURE (0x01)
    pub fn endpoint_clear_feature(
        &mut self,
        cls: &mut C,
        endpoint: u8,
        feature: u16,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device().endpoint(),
            1,
            feature,
            endpoint as u16,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Endpoint Request: SET_FEATURE (0x03)
    pub fn endpoint_set_feature(
        &mut self,
        cls: &mut C,
        endpoint: u8,
        feature: u16,
    ) -> core::result::Result<(), AnyUsbError> {
        self.control_write(
            cls,
            CtrRequestType::to_device().endpoint(),
            3,
            feature,
            endpoint as u16,
            0,
            &[],
        )
        .and(Ok(()))
    }

    /// Standard Endpoint Request: SYNCH_FRAME (0x0c)
    pub fn endpoint_synch_frame(
        &mut self,
        cls: &mut C,
        endpoint: u8,
    ) -> core::result::Result<u16, AnyUsbError> {
        let data = self.control_read(
            cls,
            CtrRequestType::to_host().endpoint(),
            12,
            0,
            endpoint as u16,
            2,
        )?;
        if data.len() != 2 {
            return Err(AnyUsbError::EP0BadGetStatusSize);
        }

        let res = data.try_into().map_err(|_| AnyUsbError::DataConversion)?;
        Ok(u16::from_le_bytes(res))
    }

    /// Setup device approximately as Host would do.
    ///
    /// This gets some standard descriptors from the device
    /// and performs standard configuration - sets
    /// Device address and sets Device configuration
    /// to `1`.
    ///
    /// This is performed automatically unless disabled
    /// by `UsbDeviceCtx`.
    ///
    /// USB reset during enumeration is not performed.
    pub fn setup(&mut self, cls: &mut C) -> core::result::Result<(), AnyUsbError> {
        let mut vec;

        // get device descriptor for max ep0 size
        // we ignore result.
        self.device_get_descriptor(cls, 1, 0, 0, 64)?;

        // todo: reset device

        // set address
        self.device_set_address(cls, X::ADDRESS)?;
        if self.dev.bus().get_address() != X::ADDRESS {
            return Err(AnyUsbError::SetAddressFailed);
        }

        // get device descriptor again
        let devd = self.device_get_descriptor(cls, 1, 0, 0, 18)?;

        // get configuration descriptor for size
        vec = self.device_get_descriptor(cls, 2, 0, 0, 9)?;
        let conf_desc_len = u16::from_le_bytes([vec[2], vec[3]]);

        // get configuration descriptor
        // we ignore result.
        self.device_get_descriptor(cls, 2, 0, 0, conf_desc_len)?;

        // get string languages
        vec = self.device_get_descriptor(cls, 3, 0, 0, 255)?;
        let lang_id = u16::from_le_bytes([vec[2], vec[3]]);

        // get string descriptors from device descriptor
        for sid in devd[14..17].iter() {
            if *sid != 0 {
                self.device_get_descriptor(cls, 3, *sid, lang_id, 255)?;
                //println!("========== {:?} {}", vec, sid);
            }
        }

        // set configuration
        self.device_set_configuration(cls, 1)?;

        Ok(())
    }
}
