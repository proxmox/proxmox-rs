use std::cell::UnsafeCell;
use std::mem::MaybeUninit;

#[derive(Debug)]
pub(crate) struct RawSharedMutex {
    inner: UnsafeCell<libc::pthread_mutex_t>,
}

unsafe impl Send for RawSharedMutex {}
unsafe impl Sync for RawSharedMutex {}

impl RawSharedMutex {

    pub const fn uninitialized() -> Self {
        Self { inner: UnsafeCell::new(libc::PTHREAD_MUTEX_INITIALIZER) }
    }

    #[inline]
    pub unsafe fn init(&mut self) {
       let mut attr = MaybeUninit::<libc::pthread_mutexattr_t>::uninit();
        cvt_nz(libc::pthread_mutexattr_init(attr.as_mut_ptr())).unwrap();
        let attr = PthreadMutexAttr(&mut attr);
        cvt_nz(libc::pthread_mutexattr_settype(attr.0.as_mut_ptr(), libc::PTHREAD_MUTEX_NORMAL))
            .unwrap();
        cvt_nz(libc::pthread_mutexattr_setpshared(attr.0.as_mut_ptr(), libc::PTHREAD_PROCESS_SHARED))
            .unwrap();
        cvt_nz(libc::pthread_mutexattr_setrobust(attr.0.as_mut_ptr(), libc::PTHREAD_MUTEX_ROBUST))
            .unwrap();
        cvt_nz(libc::pthread_mutex_init(self.inner.get(), attr.0.as_ptr())).unwrap();
    }

    #[inline]
    pub unsafe fn lock(&self) {
        let mut r = libc::pthread_mutex_lock(self.inner.get());
        if r == libc::EOWNERDEAD {
            r = libc::pthread_mutex_consistent(self.inner.get());
        }
       
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        let r = libc::pthread_mutex_unlock(self.inner.get());
        debug_assert_eq!(r, 0);
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        let mut r = libc::pthread_mutex_trylock(self.inner.get());
        if r == libc::EOWNERDEAD {
            r = libc::pthread_mutex_consistent(self.inner.get());
        }

        r == 0
    }
}


// Note: copied from rust std::sys::unix::cvt_nz
fn cvt_nz(error: libc::c_int) -> std::io::Result<()> {
    if error == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(error))
    }
}

// Copied from rust standard libs
struct PthreadMutexAttr<'a>(&'a mut MaybeUninit<libc::pthread_mutexattr_t>);

impl Drop for PthreadMutexAttr<'_> {
    fn drop(&mut self) {
        unsafe {
            let result = libc::pthread_mutexattr_destroy(self.0.as_mut_ptr());
            debug_assert_eq!(result, 0);
        }
    }
}
