# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2024-04-13

### Fixed
- Outdated documentation

## [0.2.0] - 2024-04-12

### Added
- Endpoint allocation support in EmulatedUsbBus
- Polling of all endpoints in EmulatedUsbBus
- Transfers for endpoints other than EP0
- Support of UsbClass implementations with a lifetime parameter

### Changed
- with_usb() function moved into UsbDeviceCtx trait
- Arguments and/or return type of some read/write functions

## [0.1.0] - 2024-04-06

First version.

[Unreleased]: https://github.com/vitalyvb/usbd-class-tester/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/vitalyvb/usbd-class-tester/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/vitalyvb/usbd-class-tester/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/vitalyvb/usbd-class-tester/releases/tag/v0.1.0
