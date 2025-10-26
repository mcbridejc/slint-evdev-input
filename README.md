# slint-evdev-input

Reads touch input events from a linux evdev device and converts them to slint WindowEvent.

Based on the [evdev](https://github.com/emberian/evdev) crate.

## Running tests

The tests for this crate require use of `/dev/uinput` to create virtual devices, and then access to
the created device. Typically, this requires root priveledges, or udev rules to grant access to a
group.

For example, you can add your user to the "input" group, and add the following udev rule in `/etc/udev/rules.d/99-input.rules`:

```
KERNEL=="uinput", GROUP="input", MODE:="0660"
KERNEL=="event*", GROUP="input", MODE:="0660"
```
