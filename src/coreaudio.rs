//! FFI with CoreAudio

use std::os::raw::{c_int, c_uchar, c_uint, c_void};

pub const NO_ERR: OSStatus = 0;
pub const kCFStringEncodingUTF8: c_uint = 134217984;
pub const kAudioHardwarePropertyDevices: c_uint = 1684370979;
pub const kAudioHardwarePropertyDefaultInputDevice: c_uint = 1682533920;
pub const kAudioHardwarePropertyDefaultOutputDevice: c_uint = 1682929012;
pub const kAudioDevicePropertyDeviceNameCFString: c_uint = 1819173229;
pub const kAudioDevicePropertyDeviceUID: c_uint = 1969841184;
pub const kAudioObjectPropertyScopeGlobal: c_uint = 1735159650;
pub const kAudioDevicePropertyScopeInput: c_uint = 1768845428;
pub const kAudioDevicePropertyScopeOutput: c_uint = 1869968496;
pub const kAudioDevicePropertyStreams: c_uint = 1937009955;
pub const kAudioDevicePropertyVolumeScalar: c_uint = 1987013741;
pub const kAudioDevicePropertyMute: c_uint = 1836414053;
pub const kAudioObjectPropertyElementMain: c_uint = 0;
pub const kAudioObjectSystemObject: c_uint = 1;

pub type Float32 = f32;
pub type UInt32 = c_uint;
pub type SInt32 = c_int;

pub type OSStatus = SInt32;
pub type Boolean = c_uchar;
pub type AudioObjectID = UInt32;
pub type AudioDeviceID = AudioObjectID;
pub type AudioObjectPropertySelector = UInt32;
pub type AudioObjectPropertyScope = UInt32;
pub type AudioObjectPropertyElement = UInt32;

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Default, Copy, Clone)]
pub struct AudioObjectPropertyAddress {
    pub mSelector: AudioObjectPropertySelector,
    pub mScope: AudioObjectPropertyScope,
    pub mElement: AudioObjectPropertyElement,
}

extern "C" {
    pub fn AudioObjectHasProperty(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
    ) -> Boolean;

    pub fn AudioObjectIsPropertySettable(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        outIsSettable: *mut Boolean,
    ) -> OSStatus;

    pub fn AudioObjectGetPropertyDataSize(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: UInt32,
        inQualifierData: *const c_void,
        outDataSize: *mut UInt32,
    ) -> OSStatus;

    pub fn AudioObjectGetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: UInt32,
        // Nullable. Use this to pass data into a query, like an argument.
        inQualifierData: *const c_void,
        // on entry indicates the size of the buffer pointed to by
        // outData and on exit indicates how much of the buffer was used.
        ioDataSize: *mut UInt32,
        outData: *mut c_void,
    ) -> OSStatus;

    pub fn AudioObjectSetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: UInt32,
        inQualifierData: *const c_void,
        inDataSize: UInt32,
        inData: *const c_void,
    ) -> OSStatus;
}
