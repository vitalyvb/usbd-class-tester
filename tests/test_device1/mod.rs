//! `TestUsbClass` implementation for a test `UsbClass`
use usb_device::{
    bus::{InterfaceNumber, StringIndex, UsbBus, UsbBusAllocator},
    class::UsbClass,
    control, LangID,
};

pub struct TestUsbClass {
    pub iface: InterfaceNumber,
    pub interface_string: StringIndex,
    pub byte: u8,
    pub alt_setting: u8,
}

impl TestUsbClass {
    pub fn new<B: UsbBus>(alloc: &UsbBusAllocator<B>) -> Self {
        Self {
            iface: alloc.interface(),
            interface_string: alloc.string(),
            byte: 0,
            alt_setting: 0,
        }
    }
}

impl<B: UsbBus> UsbClass<B> for TestUsbClass {
    fn control_in(&mut self, xfer: usb_device::class::ControlIn<B>) {
        let req = xfer.request();

        if req.request_type != control::RequestType::Class {
            return;
        }

        if req.recipient != control::Recipient::Interface {
            return;
        }

        if req.index != u8::from(self.iface) as u16 {
            return;
        }

        match req.request {
            1 => {
                let status: [u8; 3] = [1, 2, self.byte];
                xfer.accept_with(&status).ok();
            }
            _ => {
                xfer.reject().ok();
            }
        }
    }

    fn control_out(&mut self, xfer: usb_device::class::ControlOut<B>) {
        let req = xfer.request();

        if req.request_type != control::RequestType::Class {
            return;
        }

        if req.recipient != control::Recipient::Interface {
            return;
        }

        if req.index != u8::from(self.iface) as u16 {
            return;
        }

        let data = xfer.data();
        match req.request {
            2 => {
                if data.len() > 0 {
                    self.byte = data[0];
                    xfer.accept().ok();
                } else {
                    xfer.reject().ok();
                }
            }
            _ => {
                xfer.reject().ok();
            }
        }
    }

    fn get_alt_setting(&mut self, interface: InterfaceNumber) -> Option<u8> {
        if interface == self.iface {
            Some(self.alt_setting)
        } else {
            None
        }
    }

    fn set_alt_setting(&mut self, interface: InterfaceNumber, alternative: u8) -> bool {
        if interface == self.iface {
            self.alt_setting = alternative;
            true
        } else {
            false
        }
    }

    fn get_configuration_descriptors(
        &self,
        writer: &mut usb_device::descriptor::DescriptorWriter,
    ) -> usb_device::Result<()> {
        writer.interface_alt(
            self.iface,
            0,
            0xff, // Vendor Specific
            0x10,
            0x12,
            Some(self.interface_string),
        )?;

        writer.write(200, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])?;

        Ok(())
    }

    fn get_string(
        &self,
        index: usb_device::bus::StringIndex,
        lang_id: usb_device::prelude::LangID,
    ) -> Option<&str> {
        if index == self.interface_string && (lang_id == LangID::EN_US || u16::from(lang_id) == 0) {
            return Some("InterfaceString");
        }
        None
    }
}
