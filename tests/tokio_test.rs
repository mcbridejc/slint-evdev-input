#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_event_stream() {
    use std::time::Duration;

    use evdev::{
        AbsInfo, AbsoluteAxisCode, AttributeSet, EventType, InputEvent, KeyCode, UinputAbsSetup,
        uinput::VirtualDevice,
    };
    use slint_evdev_input::SlintEventsWrapper;

    const WIDTH: i32 = 320;
    const HEIGHT: i32 = 240;
    use slint::{
        LogicalPosition,
        platform::{PointerEventButton, WindowEvent},
    };

    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_TOUCH);
    let mut vdev = VirtualDevice::builder()
        .unwrap()
        .name("test_button_down_blocking")
        .with_absolute_axis(&UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_X,
            AbsInfo::new(0, 0, WIDTH, 0, 0, 1),
        ))
        .unwrap()
        .with_absolute_axis(&UinputAbsSetup::new(
            AbsoluteAxisCode::ABS_Y,
            AbsInfo::new(0, 0, HEIGHT, 0, 0, 1),
        ))
        .unwrap()
        .with_keys(&keys)
        .unwrap()
        .build()
        .unwrap();

    // Fetch name.
    let dev_path = vdev
        .enumerate_dev_nodes_blocking()
        .unwrap()
        .map(|p| p.unwrap())
        .next()
        .unwrap();

    std::thread::sleep(Duration::from_millis(100));

    println!("Opening {dev_path:?}");
    let mut stream = SlintEventsWrapper::new(dev_path, 1.0)
        .unwrap()
        .into_event_stream()
        .expect("Failed to open device. Do you have permissions to access /dev/input/eventX?");

    // Button down at (120, 12)
    vdev.emit(&[
        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 120),
        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, 12),
        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.code(), 1),
    ])
    .unwrap();
    // Button move to (122, 13)
    vdev.emit(&[
        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 122),
        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, 13),
    ])
    .unwrap();
    // Button move (y-only) to (122, 14)
    vdev.emit(&[InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_Y.0,
        14,
    )])
    .unwrap();
    // Button up
    vdev.emit(&[InputEvent::new(
        EventType::KEY.0,
        KeyCode::BTN_TOUCH.code(),
        0,
    )])
    .unwrap();

    let mut slint_events = Vec::new();
    // Read events until we timeout
    while let Ok(event) =
        tokio::time::timeout(Duration::from_millis(200), stream.next_event()).await
    {
        slint_events.push(event.unwrap());
    }

    assert_eq!(
        vec![
            WindowEvent::PointerPressed {
                position: LogicalPosition { x: 120.0, y: 12.0 },
                button: PointerEventButton::Left
            },
            WindowEvent::PointerMoved {
                position: LogicalPosition { x: 122.0, y: 13.0 }
            },
            WindowEvent::PointerMoved {
                position: LogicalPosition { x: 122.0, y: 14.0 }
            },
            WindowEvent::PointerReleased {
                position: LogicalPosition { x: 122.0, y: 14.0 },
                button: PointerEventButton::Left
            },
        ],
        slint_events
    );
}
