//! An example how to "extend"/add helper methods to
//! `Device` by using a module that implements a new
//! trait for `Device`.
mod test_device1;
use test_device1::*;

mod helper_example;
use helper_example::*;

use usbd_class_tester::prelude::*;

use usb_device::bus::UsbBusAllocator;

struct TestCtx {}

impl UsbDeviceCtx<EmulatedUsbBus, TestUsbClass> for TestCtx {
    fn create_class<'a>(
        &mut self,
        alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<TestUsbClass> {
        Ok(TestUsbClass::new(&alloc))
    }
}

#[test]
fn test_custom_device_get_status_set_self_powered() {
    with_usb(TestCtx {}, |mut cls, mut dev| {
        dev.usb_dev().set_self_powered(true);

        let status = dev.device_get_status(&mut cls).expect("result");
        assert_eq!(status, 1);

        let vec = dev.custom_get_status(&mut cls).expect("vec");
        assert_eq!(vec, [1, 0]);

        dev.usb_dev().set_self_powered(false);

        let vec = dev.custom_get_status(&mut cls).expect("vec");
        assert_eq!(vec, [0, 0]);
    })
    .expect("with_usb");
}
