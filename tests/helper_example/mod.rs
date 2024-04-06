//! An example how to "extend"/add helper methods to
//! `Device` by implementing a new trait for `Device`
//! in a separate module.
use usbd_class_tester::prelude::*;
use usb_device::class::UsbClass;

pub trait DeviceExt<T> {
    fn custom_get_status(&mut self, cls: &mut T) -> core::result::Result<Vec<u8>, AnyUsbError>;
}

impl<'a, T, M> DeviceExt<T> for Device<'a, T, M>
where
    T: UsbClass<EmulatedUsbBus>,
    M: UsbDeviceCtx<EmulatedUsbBus, T>,
{
    fn custom_get_status(&mut self, cls: &mut T) -> core::result::Result<Vec<u8>, AnyUsbError> {
        let res = self.device_get_status(cls)?;
        Ok(res.to_le_bytes().into())
    }
}
