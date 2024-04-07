use std::ffi::c_void;

use windows::Win32::System::Threading::*;

///  EcoQoS
///  Turn EXECUTION_SPEED throttling on.
pub fn set_eco_mode_for_current_process() -> Result<(), String> {
    unsafe {
        //  ControlMask selects the mechanism and StateMask declares which mechanism should be on or off.
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        };

        let state_pointer: *const c_void = std::ptr::addr_of!(state).cast();

        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            state_pointer,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    }
    .map_err(|err| format!("{:?} {:?}", err.message(), err.code()))
}

// TODO: fix this
