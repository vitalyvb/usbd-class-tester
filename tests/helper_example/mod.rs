//! An example how to "extend"/add helper methods to
//! `Device` by implementing a new trait for `Device`
//! in a separate module.
use usb_device::class::UsbClass;
use usbd_class_tester::prelude::*;

pub trait DeviceExt<T> {
    fn custom_get_status(&mut self, cls: &mut T) -> core::result::Result<Vec<u8>, AnyUsbError>;
}

impl<'a, C, M> DeviceExt<C> for Device<'a, C, M>
where
    C: UsbClass<EmulatedUsbBus>,
    M: UsbDeviceCtx<C<'a> = C>,
{
    fn custom_get_status(&mut self, cls: &mut C) -> core::result::Result<Vec<u8>, AnyUsbError> {
        let res = self.device_get_status(cls)?;
        Ok(res.to_le_bytes().into())
    }
}
