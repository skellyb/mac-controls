//! This repo helped me sort out how to work with CoreAudio
//! https://github.com/ewrobinson/ERVolumeAdjust

use core_foundation::{
    base::FromVoid,
    string::{CFString, CFStringRef},
};
use std::collections::HashSet;
use std::os::raw::c_void;
use std::{borrow::BorrowMut, cell::RefCell};

use crate::coreaudio::*;

const ZERO: f32 = 0.0;
const FULL: f32 = 1.0;

#[derive(Debug)]
pub struct AudioState {
    active_input: Option<usize>,
    active_output: Option<usize>,
    devices: Vec<Device>,
    mutes: Vec<AudioDeviceID>,
}

#[derive(Debug)]
pub struct Device {
    pub id: AudioDeviceID,
    pub uid: String,
    pub name: String,
    pub input: RefCell<Volume>,
    pub output: RefCell<Volume>,
}

#[derive(Debug)]
pub struct Volume {
    pub enabled: bool,
    pub level: f32,
    pub cache: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Input,
    Output,
}

/// AudioState API
impl AudioState {
    /// Init new AudioState and sync with OS.
    pub fn new() -> Self {
        let mut audio = AudioState {
            active_input: None,
            active_output: None,
            devices: Vec::new(),
            mutes: Vec::new(),
        };
        audio.update();
        audio
    }

    /// Checks state against the OS, making updates where needed.
    pub fn update(&mut self) {
        let ids = device_ids();
        let all = HashSet::<_>::from_iter(ids.into_iter());
        let curr = HashSet::from_iter(self.devices.iter().map(|d| d.id));

        // update existing devices
        for id in all.intersection(&curr) {
            let is_muted = self.mutes.contains(id);
            if let Some(device) = self.devices.iter_mut().find(|d| d.id == *id) {
                let (sys_vol_in, sys_vol_out) = volume_level(&id);
                if let Some(level) = sys_vol_in {
                    update_channel(id, &device.input, &mut self.mutes, level, is_muted);
                }
                if let Some(level) = sys_vol_out {
                    update_channel(id, &device.output, &mut self.mutes, level, is_muted);
                }
                self.mute_check(id);
            }
        }

        // add/remove
        for id in all.symmetric_difference(&curr) {
            if all.contains(id) {
                // add new device
                let (vol_in, vol_out) = volume_level(&id);
                self.devices.push(Device {
                    id: *id,
                    uid: device_uid(&id),
                    name: device_name(&id),
                    input: RefCell::new(Volume {
                        enabled: vol_in.is_some(),
                        level: vol_in.unwrap_or(ZERO),
                        cache: vol_in.unwrap_or(ZERO),
                    }),
                    output: RefCell::new(Volume {
                        enabled: vol_out.is_some(),
                        level: vol_out.unwrap_or(ZERO),
                        cache: vol_out.unwrap_or(ZERO),
                    }),
                });
                self.mute_check(id);
            } else {
                // remove
                if let Some(i) = self.devices.iter().position(|d| d.id == *id) {
                    self.devices.remove(i);
                }
                if let Some(i) = self.mutes.iter().position(|m_id| *m_id == *id) {
                    self.mutes.remove(i);
                }
            }
        }

        // Check which devices are selected
        if let Some(i) = self
            .devices
            .iter()
            .position(|d| d.id == default_device(Channel::Input))
        {
            self.active_input = Some(i);
        }
        if let Some(i) = self
            .devices
            .iter()
            .position(|d| d.id == default_device(Channel::Output))
        {
            self.active_output = Some(i);
        }
    }

    /// Get a sorted list of audio devices (active_in, active_out, muted, device).
    pub fn device_list(&self) -> Vec<(bool, bool, bool, &Device)> {
        let mut list: Vec<(bool, bool, bool, &Device)> = self
            .devices
            .iter()
            .enumerate()
            .map(|(i, d)| {
                (
                    self.active_input == Some(i),
                    self.active_output == Some(i),
                    self.mutes.contains(&d.id),
                    d,
                )
            })
            .collect();
        list.sort_by_key(|(_, _, _, d)| &d.name);
        list
    }

    /// Fetch a devices input state -> (volume, muted)
    pub fn input(&self, id: &AudioDeviceID) -> Option<(f32, bool)> {
        if let Some(device) = self.devices.iter().find(|d| d.id == *id) {
            let vol = device.input.borrow();
            match vol.enabled {
                true => Some((vol.level, self.mutes.contains(id))),
                false => None,
            }
        } else {
            None
        }
    }

    /// Fetch a devices output state -> (level, muted)
    pub fn output(&self, id: &AudioDeviceID) -> Option<(f32, bool)> {
        if let Some(device) = self.devices.iter().find(|d| d.id == *id) {
            let vol = device.output.borrow();
            match vol.enabled {
                true => Some((vol.level, self.mutes.contains(id))),
                false => None,
            }
        } else {
            None
        }
    }

    /// Adjust volume by variable amount (with max/min of 1.0/0.0)
    pub fn move_volume(&mut self, channel: Channel, amount: f32) {
        {
            let (id, mut vol_ref) = match channel {
                Channel::Input if self.active_input.is_some() => {
                    let device = &self.devices[self.active_input.unwrap()];
                    (device.id, device.input.borrow_mut())
                }
                Channel::Output if self.active_output.is_some() => {
                    let device = &self.devices[self.active_output.unwrap()];
                    (device.id, device.output.borrow_mut())
                }
                _ => return,
            };
            if vol_ref.enabled {
                let mut next_level = vol_ref.level + amount;
                next_level = if next_level < ZERO { ZERO } else { next_level };
                next_level = if next_level > FULL { FULL } else { next_level };
                vol_ref.level = next_level;
                vol_ref.cache = next_level;
                set_volume(&id, channel, next_level);
            }
        }
        self.update();
    }

    // Toggle workaround mute for input or output.
    pub fn toggle_mute(&mut self, channel: Channel) {
        {
            let (id, vol_state) = match channel {
                Channel::Input if self.active_input.is_some() => {
                    let device = &self.devices[self.active_input.unwrap()];
                    (device.id, device.input.borrow())
                }
                Channel::Output if self.active_output.is_some() => {
                    let device = &self.devices[self.active_output.unwrap()];
                    (device.id, device.output.borrow())
                }
                _ => return,
            };
            if vol_state.enabled {
                if self.mutes.contains(&id) {
                    set_volume(&id, channel, vol_state.cache);
                } else {
                    set_volume(&id, channel, ZERO);
                }
            }
        }
        self.update();
    }
}

impl AudioState {
    /// Monterey introduced a bug where a mute change is applied to both input
    /// and output of a bluetooth device, making it impossible to mute the mic
    /// without muting speakers.
    ///
    /// Here we check if a new system mute is set, if so, takeover control.
    /// Save the current volume level, set volume to 0 if muted, and unmute
    /// the system. We use our cached volume level to unmute.
    fn mute_check(&mut self, id: &AudioDeviceID) {
        let (mute_in, mute_out) = device_mutes(&id);
        let new_in = mute_in.is_some() && mute_in.unwrap();
        let new_out = mute_out.is_some() && mute_out.unwrap();
        if new_in || new_out {
            let chan: Channel;
            let mut chan_state = if mute_in.is_some() {
                chan = Channel::Input;
                // TODO: ugly access
                self.devices
                    .iter_mut()
                    .find(|d| d.id == *id)
                    .unwrap()
                    .input
                    .borrow_mut()
            } else if mute_out.is_some() {
                chan = Channel::Output;
                // TODO: ugly access
                self.devices
                    .iter_mut()
                    .find(|d| d.id == *id)
                    .unwrap()
                    .output
                    .borrow_mut()
            } else {
                return;
            };
            // set volume to 0 (sys and state)
            set_volume(&id, chan, ZERO);
            // cache current volume level
            let vol_ref = chan_state.borrow_mut();
            vol_ref.cache = vol_ref.level;
            vol_ref.level = ZERO;

            // unmute system
            set_mute(&id, chan, false);
            // add ID to mutes state
            if !self.mutes.contains(&id) {
                self.mutes.push(*id);
            }
        }
    }
}

fn update_channel(
    id: &u32,
    vol_state: &RefCell<Volume>,
    mutes: &mut Vec<u32>,
    level: f32,
    is_muted: bool,
) {
    let mut v_ref = vol_state.borrow_mut();
    v_ref.enabled = true;
    v_ref.level = level;
    if level > ZERO && is_muted {
        // volume raised, remove from mutes
        if let Some(i) = mutes.iter().position(|mid| *mid == *id) {
            mutes.remove(i);
        }
    } else if level == ZERO && !is_muted {
        // volume dropped to zero, add to mutes
        mutes.push(*id);
    }
}


/// First get the size of the "devices" data. Divide that by the size of a u32
/// to get the number of devices. Finally, fetch the data in a u32 vec.
fn device_ids() -> Vec<u32> {
    let prop_size = query_size(
        &kAudioObjectSystemObject,
        kAudioHardwarePropertyDevices,
        kAudioObjectPropertyScopeGlobal,
    )
    .expect("Query audio object size");
    let num_devices = prop_size as usize / std::mem::size_of::<AudioDeviceID>();
    if num_devices == 0 {
        return vec![];
    }
    query_audio_object::<UInt32>(
        &kAudioObjectSystemObject,
        kAudioHardwarePropertyDevices,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
        num_devices,
    )
}

/// Get device's human readable name.
fn device_name(id: &u32) -> String {
    unsafe {
        // Get pointer bytes, then throw out head and tail, converting the
        // body of bytes to a CFStringRef
        let name_buf = query_audio_object::<u8>(
            id,
            kAudioDevicePropertyDeviceNameCFString,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
            8,
        );
        let (_, name_ref, _) = name_buf.align_to::<CFStringRef>();
        ref_to_string(name_ref[0])
    }
}

/// Get device's unique ID string.
fn device_uid(id: &u32) -> String {
    unsafe {
        // Get pointer bytes, then throw out head and tail, converting the
        // body of bytes to a CFStringRef (a typed pointer)
        let uid_buf = query_audio_object::<u8>(
            id,
            kAudioDevicePropertyDeviceUID,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
            8,
        );
        let (_, uid_ref, _) = uid_buf.align_to::<CFStringRef>();
        ref_to_string(uid_ref[0])
    }
}

/// Get current input/output levels for device.
fn volume_level(id: &u32) -> (Option<f32>, Option<f32>) {
    let out_chans = query_size(
        id,
        kAudioDevicePropertyStreams,
        kAudioDevicePropertyScopeOutput,
    )
    .unwrap();
    let in_chans = query_size(
        id,
        kAudioDevicePropertyStreams,
        kAudioDevicePropertyScopeInput,
    )
    .unwrap();

    // TODO: Check what other channels are doing
    // iterate through channels checking if it has volume
    let mut out_volume = None;
    let mut in_volume = None;
    for i in 0..out_chans {
        if query_exists(
            id,
            kAudioDevicePropertyVolumeScalar,
            kAudioDevicePropertyScopeOutput,
            i,
        ) {
            let vol_buf = query_audio_object::<Float32>(
                id,
                kAudioDevicePropertyVolumeScalar,
                kAudioDevicePropertyScopeOutput,
                i,
                1,
            );
            out_volume = Some(vol_buf[0]);
            break;
        }
    }
    for i in 0..in_chans {
        if query_exists(
            id,
            kAudioDevicePropertyVolumeScalar,
            kAudioDevicePropertyScopeInput,
            i,
        ) {
            let vol_buf = query_audio_object::<Float32>(
                id,
                kAudioDevicePropertyVolumeScalar,
                kAudioDevicePropertyScopeInput,
                i,
                1,
            );
            in_volume = Some(vol_buf[0]);
            break;
        }
    }
    (in_volume, out_volume)
}

/// Get (input, output) mute state for a device
fn device_mutes(id: &u32) -> (Option<bool>, Option<bool>) {
    let mut in_mute = None;
    let mut out_mute = None;
    if query_exists(
        id,
        kAudioDevicePropertyMute,
        kAudioDevicePropertyScopeOutput,
        kAudioObjectPropertyElementMain,
    ) {
        let muted = query_audio_object::<UInt32>(
            id,
            kAudioDevicePropertyMute,
            kAudioDevicePropertyScopeOutput,
            kAudioObjectPropertyElementMain,
            1,
        );
        out_mute = Some(muted[0] == 1);
    }

    if query_exists(
        id,
        kAudioDevicePropertyMute,
        kAudioDevicePropertyScopeInput,
        kAudioObjectPropertyElementMain,
    ) {
        let muted = query_audio_object::<UInt32>(
            id,
            kAudioDevicePropertyMute,
            kAudioDevicePropertyScopeInput,
            kAudioObjectPropertyElementMain,
            1,
        );
        in_mute = Some(muted[0] == 1);
    }
    (in_mute, out_mute)
}

/// Find currently active device
fn default_device(signal: Channel) -> AudioObjectID {
    let selector = match signal {
        Channel::Input => kAudioHardwarePropertyDefaultInputDevice,
        Channel::Output => kAudioHardwarePropertyDefaultOutputDevice,
    };
    let d = query_audio_object::<UInt32>(
        &kAudioObjectSystemObject,
        selector,
        kAudioObjectPropertyScopeGlobal,
        kAudioObjectPropertyElementMain,
        1,
    );
    d[0]
}

/// Change device's volume
fn set_volume(id: &u32, channel: Channel, volume: f32) {
    let scope = match channel {
        Channel::Input => kAudioDevicePropertyScopeInput,
        Channel::Output => kAudioDevicePropertyScopeOutput,
    };

    // Number of channels
    let channels = query_size(id, kAudioDevicePropertyStreams, scope).unwrap();

    // Iterate through channels, check if settable, then set
    for i in 0..channels {
        if query_settable(id, kAudioDevicePropertyVolumeScalar, scope, i) {
            set_audio_object_prop(id, kAudioDevicePropertyVolumeScalar, scope, i, volume).unwrap();
        }
    }
}

/// Set device's mute state
fn set_mute(id: &u32, channel: Channel, enabled: bool) {
    let mute_val: UInt32 = if enabled { 1 } else { 0 };
    let scope = match channel {
        Channel::Input => kAudioDevicePropertyScopeInput,
        Channel::Output => kAudioDevicePropertyScopeOutput,
    };
    set_audio_object_prop(
        id,
        kAudioDevicePropertyMute,
        scope,
        kAudioObjectPropertyElementMain,
        mute_val,
    )
    .unwrap();
}

/// Check if audio property exists on object
fn query_exists(
    object_id: &AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    element: UInt32,
) -> bool {
    let prop_address = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    };
    unsafe { AudioObjectHasProperty(object_id.clone(), &prop_address) > 0 }
}

/// Query size of a property's buffer
fn query_size(
    object_id: &AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Result<UInt32, ()> {
    let mut prop_size: UInt32 = 0;
    let prop_address = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    };
    unsafe {
        if AudioObjectGetPropertyDataSize(
            object_id.clone(),
            &prop_address,
            0,
            std::ptr::null(),
            &mut prop_size,
        ) == NO_ERR
        {
            Ok(prop_size)
        } else {
            Err(())
        }
    }
}

/// Query an audio property
fn query_audio_object<T: Clone + Default + Sized>(
    object_id: &AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    element: AudioObjectPropertyElement,
    len: usize,
) -> Vec<T> {
    // Size of the buffer going in
    let mut data_size: UInt32 = (std::mem::size_of::<T>() * len) as UInt32;
    // This struct is the "query"
    let prop_address = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    };
    unsafe {
        let buf = buf_ptr::<T>(len);
        // TODO: handle possible OSStatus error? Like set_audio_object_prop
        AudioObjectGetPropertyData(
            object_id.clone(),
            &prop_address,
            0,
            std::ptr::null(),
            &mut data_size,
            buf,
        );
        let result_len = data_size / std::mem::size_of::<T>() as UInt32;
        vec_from_ptr::<T>(buf, result_len as usize)
    }
}

fn query_settable(
    object_id: &AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    element: UInt32,
) -> bool {
    let mut settable: Boolean = 0;
    let prop_address = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    };
    unsafe {
        AudioObjectIsPropertySettable(object_id.clone(), &prop_address, &mut settable);
    }
    settable > 0
}

fn set_audio_object_prop<T: Clone + Default + Sized>(
    object_id: &AudioObjectID,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    element: AudioObjectPropertyElement,
    input: T,
) -> Result<(), String> {
    let data_size = std::mem::size_of::<T>() as UInt32;
    let prop_address = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    };
    unsafe {
        if AudioObjectSetPropertyData(
            object_id.clone(),
            &prop_address,
            0,
            std::ptr::null(),
            data_size,
            std::ptr::addr_of!(input) as *const c_void,
        ) == NO_ERR
        {
            Ok(())
        } else {
            Err("Unable to set audio object prop".to_string())
        }
    }
}

fn ref_to_string(cf_str_ref: CFStringRef) -> String {
    unsafe {
        let cfs = CFString::from_void(cf_str_ref as *const c_void);
        cfs.to_string()
    }
}

fn buf_ptr<T: Clone + Default>(len: usize) -> *mut c_void {
    let mut v: Vec<T> = vec![];
    v.reserve_exact(len);
    v.resize_with(len, Default::default);
    let mut boxed_buffer = v.into_boxed_slice();
    let data = boxed_buffer.as_mut_ptr();
    std::mem::forget(boxed_buffer);
    data as *mut c_void
}

fn vec_from_ptr<T>(ptr: *mut c_void, len: usize) -> Vec<T> {
    unsafe {
        let v: Vec<T> = Vec::from_raw_parts(ptr as *mut T, len, len);
        v
    }
}
