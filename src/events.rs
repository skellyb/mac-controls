use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, EventField,
};

#[derive(Debug)]
pub enum Action {
    KeyUp {
        key_code: i64,
        modifiers: ModifierKeys,
    },
    KeyDown {
        key_code: i64,
        repeating: bool,
        modifiers: ModifierKeys,
    },
    Modifier {
        modifiers: ModifierKeys,
    },
    ModeSwitch(UiMode),
    SelectNext,
    SelectPrev,
    VolumeUp,
    VolumeDown,
    ToggleMute,
    Poll,
    Exit,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ModifierKeys {
    pub caps_lock: bool,
    pub shift: bool,
    pub control: bool,
    pub option: bool,
    pub command: bool,
    pub func: bool,
}

impl ModifierKeys {
    pub fn list_active(&self) -> Vec<String> {
        let mut out = vec![];
        if self.func {
            out.push("fn".to_string());
        }
        if self.caps_lock {
            out.push("caps lock".to_string());
        }
        if self.shift {
            out.push("shift".to_string());
        }
        if self.control {
            out.push("control".to_string());
        }
        if self.option {
            out.push("option".to_string());
        }
        if self.command {
            out.push("command".to_string());
        }
        out
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum UiMode {
    View,
    EditInput,
    EditOutput,
}

#[repr(C)]
enum IOHIDRequestType {
    IOHIDRequestTypePostEvent,
    IOHIDRequestTypeListenEvent,
}

extern "C" {
    fn IOHIDRequestAccess(requestType: IOHIDRequestType) -> bool;
}

/// Request accessibility and input monitoring permissions from macOS
pub fn request_accessibility_access() -> bool {
    unsafe {
        let has_access = IOHIDRequestAccess(IOHIDRequestType::IOHIDRequestTypeListenEvent);
        let has_input = IOHIDRequestAccess(IOHIDRequestType::IOHIDRequestTypePostEvent);
        has_access && has_input
    }
}

pub fn event_tap<F>(handler: F) -> Result<(), String>
where
    F: Fn(Action),
{
    let curr_loop = CFRunLoop::get_current();

    match CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        |_, event_type, event| {
            let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
            let repeating =
                event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) > 0;
            // TODO: need to check flags on init, not waiting for first event.
            //       usecase: caps_lock might already be on
            let flags = event.get_flags();
            let modifiers = ModifierKeys {
                caps_lock: flags.contains(CGEventFlags::CGEventFlagAlphaShift),
                shift: flags.contains(CGEventFlags::CGEventFlagShift),
                control: flags.contains(CGEventFlags::CGEventFlagControl),
                option: flags.contains(CGEventFlags::CGEventFlagAlternate),
                command: flags.contains(CGEventFlags::CGEventFlagCommand),
                func: flags.contains(CGEventFlags::CGEventFlagSecondaryFn),
            };
            match event_type {
                CGEventType::KeyDown => handler(Action::KeyDown {
                    key_code,
                    modifiers,
                    repeating,
                }),
                CGEventType::KeyUp => handler(Action::KeyUp {
                    key_code,
                    modifiers,
                }),
                CGEventType::FlagsChanged => handler(Action::Modifier { modifiers }),
                _ => (),
            }
            None
        },
    ) {
        Ok(tap) => unsafe {
            let loop_source = tap
                .mach_port
                .create_runloop_source(0)
                .expect("Connect to run loop.");
            curr_loop.add_source(&loop_source, kCFRunLoopCommonModes);
            tap.enable();
            CFRunLoop::run_current();
            Ok(())
        },
        Err(_) => Err("Failed to create event tap.".to_string()),
    }
}
