mod test_device1;
use test_device1::*;

use usbd_class_tester::prelude::*;

use usb_device::{
    bus::{UsbBus, UsbBusAllocator},
    class::UsbClass,
    device::UsbDeviceState,
};

#[derive(Default)]
struct TestCtx {
    skip_setup: bool,
}

impl TestCtx {
    fn new() -> Self {
        Self::default()
    }
    fn no_setup() -> Self {
        Self { skip_setup: true }
    }
}

impl UsbDeviceCtx for TestCtx {
    type C<'c> = TestUsbClass;
    const ADDRESS: u8 = 55;

    fn create_class<'a>(
        &mut self,
        alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<TestUsbClass> {
        Ok(TestUsbClass::new(&alloc))
    }

    fn skip_setup(&mut self) -> bool {
        self.skip_setup
    }
}

#[test]
fn test_device_get_status_set_self_powered() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        dev.usb_dev().set_self_powered(true);

        let status = dev.device_get_status(&mut cls).expect("vec");
        assert_eq!(status, 1);

        dev.usb_dev().set_self_powered(false);

        let status = dev.device_get_status(&mut cls).expect("vec");
        assert_eq!(status, 0);
    })
    .expect("with_usb");
}

#[test]
fn test_device_feature_remote_wakeup() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        dev.device_set_feature(&mut cls, 1).expect("failed");
        assert_eq!(dev.usb_dev().remote_wakeup_enabled(), true);

        dev.device_clear_feature(&mut cls, 1).expect("failed");
        assert_eq!(dev.usb_dev().remote_wakeup_enabled(), false);
    })
    .expect("with_usb");
}

#[test]
fn test_device_address_set() {
    TestCtx::new()
    .with_usb(|mut _cls, mut dev| {
        assert_eq!(dev.usb_dev().bus().get_address(), TestCtx::ADDRESS);
    })
    .expect("with_usb");
}

#[test]
fn test_device_configured() {
    TestCtx::new()
    .with_usb(|mut _cls, mut dev| {
        assert_eq!(dev.usb_dev().state(), UsbDeviceState::Configured);
    })
    .expect("with_usb");
}

#[test]
fn test_device_set_address_and_configuration() {
    TestCtx::no_setup()
    .with_usb(|mut cls, mut dev| {
        let mut cnf;

        assert_eq!(dev.usb_dev().state(), UsbDeviceState::Default);

        cnf = dev.device_get_configuration(&mut cls).expect("failed");
        assert_eq!(cnf, 0);

        assert_eq!(dev.usb_dev().state(), UsbDeviceState::Default);

        dev.device_set_address(&mut cls, TestCtx::ADDRESS)
            .expect("failed");
        assert_eq!(dev.usb_dev().state(), UsbDeviceState::Addressed);

        dev.device_set_configuration(&mut cls, 1).expect("failed");

        assert_eq!(dev.usb_dev().state(), UsbDeviceState::Configured);

        cnf = dev.device_get_configuration(&mut cls).expect("failed");
        assert_eq!(cnf, 1);
    })
    .expect("with_usb");
}

#[test]
fn test_device_get_descriptor_strings() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        let mut vec;

        let desc = |s: &str| {
            let unicode_bytes: Vec<u8> = s
                .encode_utf16()
                .map(|x| x.to_le_bytes())
                .flatten()
                .collect();
            [&[(unicode_bytes.len() + 2) as u8, 3], &unicode_bytes[..]].concat()
        };

        // get default string descriptors
        vec = dev
            .device_get_descriptor(&mut cls, 3, 1, 0x409, 255)
            .expect("vec");
        assert_eq!(vec, desc("TestManufacturer"));

        vec = dev
            .device_get_descriptor(&mut cls, 3, 2, 0x409, 255)
            .expect("vec");
        assert_eq!(vec, desc("TestProduct"));

        vec = dev
            .device_get_descriptor(&mut cls, 3, 3, 0x409, 255)
            .expect("vec");
        assert_eq!(vec, desc("TestSerial"));
    })
    .expect("with_usb");
}

#[test]
fn test_device_get_strings() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        let mut res;

        // get default string descriptors
        res = dev.device_get_string(&mut cls, 1, 0x409).expect("string");
        assert_eq!(res, "TestManufacturer");

        res = dev.device_get_string(&mut cls, 2, 0x409).expect("string");
        assert_eq!(res, "TestProduct");

        res = dev.device_get_string(&mut cls, 3, 0x409).expect("string");
        assert_eq!(res, "TestSerial");
    })
    .expect("with_usb");
}

#[test]
fn test_interface_get_status() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        let st = dev.interface_get_status(&mut cls, 0).expect("status");
        assert_eq!(st, 0);
    })
    .expect("with_usb");
}

#[test]
fn test_interface_alt_interface() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        let st = dev
            .interface_get_interface(&mut cls)
            .expect("get_interface");
        assert_eq!(st, 0);
        assert_eq!(cls.alt_setting, 0);

        dev.interface_set_interface(&mut cls, 0, 1)
            .expect("set_interface");
        assert_eq!(cls.alt_setting, 1);

        let st = dev
            .interface_get_interface(&mut cls)
            .expect("get_interface");
        assert_eq!(st, 1);
    })
    .expect("with_usb");
}

#[test]
fn test_interface_get_set_feature() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        dev.interface_set_feature(&mut cls, 0, 1)
            .expect_err("interface feature");
        dev.interface_clear_feature(&mut cls, 0, 1)
            .expect_err("interface feature");
    })
    .expect("with_usb");
}

#[test]
fn test_device_custom_control_command() {
    TestCtx::new()
    .with_usb(|mut cls, mut dev| {
        let mut vec;

        vec = dev
            .control_read(
                &mut cls,
                CtrRequestType::to_host().class().interface(),
                1,
                0,
                0,
                8,
            )
            .expect("vec");
        assert_eq!(vec, [1, 2, 0]);

        dev.control_write(
            &mut cls,
            CtrRequestType::to_device().class().interface(),
            2,
            0,
            0,
            0,
            &[],
        )
        .expect_err("stall");

        vec = dev
            .control_write(
                &mut cls,
                CtrRequestType::to_device().class().interface(),
                2,
                0,
                0,
                1,
                &[0xaa],
            )
            .expect("res");
        assert_eq!(vec, []);

        vec = dev
            .control_read(
                &mut cls,
                CtrRequestType::to_host().class().interface(),
                1,
                0,
                0,
                8,
            )
            .expect("vec");
        assert_eq!(vec, [1, 2, 0xaa]);
    })
    .expect("with_usb");
}

struct FailTestUsbClass {}

impl<B: UsbBus> UsbClass<B> for FailTestUsbClass {}

struct FailTestCtx {}

impl UsbDeviceCtx for FailTestCtx {
    type C<'c> = FailTestUsbClass;
    
    const ADDRESS: u8 = 55;

    fn create_class<'a>(
        &mut self,
        _alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<FailTestUsbClass> {
        Err(AnyUsbError::UserDefined1)
    }
}

#[test]
#[should_panic(expected = "with_usb: UserDefined1")]
fn test_create_class_fails() {
    FailTestCtx {}
    .with_usb(|mut _cls, mut _dev| {
        unreachable!("case should not run");
    })
    .expect("with_usb");
}
