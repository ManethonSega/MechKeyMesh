#[cfg(windows)]
mod windows {
    use crate::*;
    use std::ffi::c_void;

    #[repr(C)]
    struct SubscribeParameters {
        callback: unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> u32,
        context: *mut c_void,
    }

    #[link(name = "powrprof")]
    unsafe extern "system" {
        fn PowerRegisterSuspendResumeNotification(
            flags: u32,
            recipient: *mut c_void,
            registration: *mut *mut c_void,
        ) -> u32;
        fn PowerUnregisterSuspendResumeNotification(registration: *mut c_void) -> u32;
    }

    pub(crate) struct PowerRegistration {
        registration: std::sync::atomic::AtomicUsize,
        // Stable callback address, owned until notifications are unregistered.
        _context: Box<tauri::AppHandle>,
    }

    impl Drop for PowerRegistration {
        fn drop(&mut self) {
            let registration = self.registration.swap(0, Ordering::AcqRel);
            if registration != 0 {
                unsafe {
                    PowerUnregisterSuspendResumeNotification(registration as *mut c_void);
                }
            }
        }
    }

    pub(crate) fn unregister(app: &tauri::AppHandle) {
        if let Some(state) = app.try_state::<PowerRegistration>() {
            let registration = state.registration.swap(0, Ordering::AcqRel);
            if registration != 0 {
                unsafe {
                    PowerUnregisterSuspendResumeNotification(registration as *mut c_void);
                }
            }
        }
    }

    unsafe extern "system" fn callback(
        context: *mut c_void,
        kind: u32,
        _setting: *mut c_void,
    ) -> u32 {
        if context.is_null() {
            return 87;
        }
        // Never allow Rust unwinding across a native callback boundary.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let app = unsafe { &*(context as *const tauri::AppHandle) };
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            if state.background.exiting.load(Ordering::Acquire) {
                return;
            }
            match kind {
                4 => {
                    state.background.sleeping.store(true, Ordering::Release);
                    state.manager.suspend();
                    state
                        .profiles
                        .last_device_instance
                        .lock()
                        .expect("profile device lock poisoned")
                        .take();
                    stop_live_rgb_controller(&state);
                    stop_music_controller(&state);
                }
                7 | 18 => {
                    // Automatic and user resume notifications may both arrive.
                    if state.background.sleeping.swap(false, Ordering::AcqRel) {
                        state
                            .background
                            .resume_pending
                            .store(true, Ordering::Release);
                        state.manager.resume();
                    }
                }
                _ => {}
            }
        }));
        if result.is_ok() { 0 } else { 31 }
    }

    pub(crate) fn register(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let mut context = Box::new(app.clone());
        let mut parameters = SubscribeParameters {
            callback,
            context: (&mut *context as *mut tauri::AppHandle).cast(),
        };
        let mut registration = std::ptr::null_mut();
        let code = unsafe {
            PowerRegisterSuspendResumeNotification(
                2,
                (&mut parameters as *mut SubscribeParameters).cast(),
                &mut registration,
            )
        };
        if code != 0 {
            return Err(std::io::Error::from_raw_os_error(code as i32).into());
        }
        app.manage(PowerRegistration {
            registration: std::sync::atomic::AtomicUsize::new(registration as usize),
            _context: context,
        });
        Ok(())
    }
}

#[cfg(windows)]
pub(crate) use windows::{register, unregister};

#[cfg(not(windows))]
pub(crate) fn register(_app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn unregister(_app: &tauri::AppHandle) {}
