# usbd-class-tester

[![Crates.io](https://img.shields.io/crates/v/usbd-class-tester.svg)](https://crates.io/crates/usbd-class-tester) [![Docs.rs](https://docs.rs/usbd-class-tester/badge.svg)](https://docs.rs/usbd-class-tester)

A library for running tests of `usb-device` classes on
developer's system natively.

## About

Testing is difficult, and if it's even more difficult
when it involves a dedicated hardware and doing
the test manually. Often a lot of stuff needs to be
re-tested even after small code changes.

This library aims to help testing the implementation of
protocols in USB devices which are based on `usb-device`
crate by providing a means of simulating Host's accesses
to the device.

Initial implementation was done for tests in `usbd-dfu`
crate. This library is based on that idea, but extends
it a lot. For example it adds a set of convenience
functions for Control transfers, while originally this
was done via plain `u8` arrays only.

### Supported operations

* IN and OUT EP0 control transfers

### Not supported operations

Almost everything else, including but not limited to:

* Endpoints other than EP0 in `EmulatedUsbBus::poll()`
* Endpoint allocation in `EmulatedUsbBus::alloc_ep()`
* Reset
* Suspend and Resume
* Interrupt transfers
* Bulk transfers
* Iso transfers
* ...

## License

This project is licensed under [MIT License](https://opensource.org/licenses/MIT)
([LICENSE](https://github.com/vitalyvb/usbd-class-tester/blob/main/LICENSE)).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you shall be licensed as above,
without any additional terms or conditions.

## Example

The example defines an empty `UsbClass` implementation for `TestUsbClass`.
Normally this would also include things like endpoint allocations,
device-specific descriptor generation, and the code handling everything.
This is not in the scope of this example.

A minimal `TestCtx` creates `TestUsbClass` that will be passed to
a test case. In general, `TestCtx` allows some degree of environment
customization, like choosing EP0 transfer size, or redefining how
`UsbDevice` is created.

Check crate tests directory for more examples.

Also see the documentation for `usb-device`.

```
use usb_device::class_prelude::*;
use usbd_class_tester::prelude::*;

// `UsbClass` under the test.
pub struct TestUsbClass {}
impl<B: UsbBus> UsbClass<B> for TestUsbClass {}

// Context to create a testable instance of `TestUsbClass`
struct TestCtx {}
impl UsbDeviceCtx<EmulatedUsbBus, TestUsbClass> for TestCtx {
    fn create_class<'a>(
        &mut self,
        alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<TestUsbClass> {
        Ok(TestUsbClass {})
    }
}

#[test]
fn test_interface_get_status() {
    with_usb(TestCtx {}, |mut cls, mut dev| {
        let st = dev.interface_get_status(&mut cls, 0).expect("status");
        assert_eq!(st, 0);
    })
    .expect("with_usb");
}
```

