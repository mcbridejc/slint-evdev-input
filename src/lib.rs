//! Convert linux evdev input events from a touchscreen into slint WindowEvents
//!
//! # Why?
//!
//! For small embedded linux devices it may not make sense to run the normal slint event loop, but
//! to use the MinimalSoftwareRenderer to draw to a frame buffer, and implement the event loop as
//! part of the application. Most linux touch drivers provide an events interface, e.g.
//! `/etc/input/event0`, from which events can be read.
//!
//! This is a simple wrapper around the [`evdev` crate](https://crates.io/crates/evdev) to convert
//! the input events into WindowEvent structs which can be passed to slint via the
//! `dispatch_event()` method on a MinimalSoftwareWindow.
//!
//! # Caveats
//!
//! This only supports touch events: PointerPressed, PointerMoved, and PointedReleased.
//!
//! # Usage
//!
//! Can be used as a blocking call via [`fetch_events()`](SlintEventsWrapper::fetch_events), or via
//! async stream by enabling the `tokio` feature and using
//! [`into_event_stream()`](SlintEventsWrapper::into_event_stream) to create an [`EventStream`](tokio::EventStream).
//!
#![warn(missing_docs)]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
use std::path::Path;

use evdev::{AbsoluteAxisCode, EventSummary, FetchEventsSynced, KeyCode};
use slint::{
    LogicalPosition, PhysicalPosition,
    platform::{PointerEventButton, WindowEvent},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum ButtonChange {
    #[default]
    None,
    Up,
    Down,
}

/// Collect evdev events and convert them to slint events
struct Collector {
    last_position: (i32, i32),
    scale_factor: f32,
    button_change: ButtonChange,
}

impl Collector {
    pub fn new(scale_factor: f32, last_position: (i32, i32)) -> Self {
        Self {
            last_position,
            scale_factor,
            button_change: ButtonChange::None,
        }
    }

    pub fn push(&mut self, event: evdev::EventSummary) -> Option<WindowEvent> {
        match event {
            EventSummary::Synchronization(_, _, _) => {
                let button_change = self.button_change;
                self.button_change = ButtonChange::None;
                if button_change == ButtonChange::Down {
                    return Some(WindowEvent::PointerPressed {
                        position: self.last_logical_position(),
                        button: PointerEventButton::Left,
                    });
                } else if button_change == ButtonChange::Up {
                    return Some(WindowEvent::PointerReleased {
                        position: self.last_logical_position(),
                        button: PointerEventButton::Left,
                    });
                } else {
                    return Some(WindowEvent::PointerMoved {
                        position: self.last_logical_position(),
                    });
                };
            }
            EventSummary::AbsoluteAxis(_event, code, value) => match code {
                AbsoluteAxisCode::ABS_X => self.last_position.0 = value,
                AbsoluteAxisCode::ABS_Y => self.last_position.1 = value,
                _ => (),
            },
            EventSummary::Key(_event, key, value) => {
                if matches!(key, KeyCode::BTN_TOUCH) {
                    if value == 1 {
                        self.button_change = ButtonChange::Down
                    } else {
                        self.button_change = ButtonChange::Up;
                    }
                }
            }
            _ => (),
        }
        None
    }

    fn last_logical_position(&self) -> LogicalPosition {
        let (x, y) = self.last_position;
        LogicalPosition::from_physical(PhysicalPosition::new(x, y), self.scale_factor)
    }
}

/// A wrapper for evdev::Device to convert events to slint WindowEvents
///
/// Only supports single-touch touch screens
///
/// # Example
///
/// ```no_run
///  use slint_evdev_input::SlintEventsWrapper;
///
///  // Scale factor from `slint::Window::scale_factor()` for converting logical to physical pixel
///  // coordinates
///  let scale_factor = 1.0;
///  let mut slint_device = SlintEventsWrapper::new("/dev/input/event0", scale_factor).unwrap();
///
///  loop {
///      for event in slint_device.fetch_events() {
///          println!("{:?}", event);
///      }
///  }
/// ```
pub struct SlintEventsWrapper {
    device: evdev::Device,
    last_position: (i32, i32),
    scale_factor: f32,
}

impl SlintEventsWrapper {
    /// Create a new SlintEventsWrapper using the given event device path
    ///
    /// # Arguments
    ///
    /// - `device`: A path to the device (e.g. '/dev/input/event0')
    /// - `scale_factor`: The scale factor from slint for converting between logical and physical
    ///   coordinates.
    pub fn new(device: impl AsRef<Path>, scale_factor: f32) -> std::io::Result<Self> {
        let device = evdev::Device::open(device)?;
        Ok(Self {
            device,
            last_position: (0, 0),
            scale_factor,
        })
    }

    /// Fetches and returns event. This will block until events are ready.
    pub fn fetch_events<'a>(&'a mut self) -> SlintEventsIterator<'a> {
        SlintEventsIterator {
            inner: self.device.fetch_events().unwrap(),
            collector: Collector::new(self.scale_factor, self.last_position),
        }
    }

    /// Convert the wrapper into an [`EventStream`] for async reading
    ///
    /// Requires the `tokio` feature
    #[cfg(feature = "tokio")]
    pub fn into_event_stream(self) -> std::io::Result<tokio::EventStream> {
        Ok(tokio::EventStream {
            evdev_stream: self.device.into_event_stream()?,
            collector: Collector::new(self.scale_factor, self.last_position),
        })
    }
}

/// An iterator over window events which will block until a new event is ready
pub struct SlintEventsIterator<'a> {
    inner: FetchEventsSynced<'a>,
    collector: Collector,
}

impl Iterator for SlintEventsIterator<'_> {
    type Item = WindowEvent;

    fn next(&mut self) -> Option<Self::Item> {
        // Read to sync event
        loop {
            match self.inner.next() {
                Some(event) => {
                    if let Some(window_event) = self.collector.push(event.destructure()) {
                        return Some(window_event);
                    }
                }
                None => return None,
            }
        }
    }
}

#[cfg(feature = "tokio")]
mod tokio {
    use super::*;
    /// A async stream of input events
    pub struct EventStream {
        pub(super) evdev_stream: evdev::EventStream,
        pub(super) collector: Collector,
    }

    impl EventStream {
        /// Get a future for the next available event in the stream
        pub async fn next_event(&mut self) -> Result<WindowEvent, std::io::Error> {
            loop {
                let event = self.evdev_stream.next_event().await?;
                if let Some(ret) = self.collector.push(event.destructure()) {
                    return Ok(ret);
                }
            }
        }
    }
}
