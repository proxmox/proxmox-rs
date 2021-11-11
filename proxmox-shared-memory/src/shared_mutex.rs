use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::Init;
use crate::raw_shared_mutex::RawSharedMutex;

#[derive(Debug)]
#[repr(C)]
pub struct SharedMutex<T: ?Sized> {
    inner: RawSharedMutex,
    data: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for SharedMutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for SharedMutex<T> {}

impl <T: Init> Init for SharedMutex<T> {

    fn initialize(this: &mut MaybeUninit<SharedMutex<T>>) {

        let me = unsafe { &mut *this.as_mut_ptr() };

        me.inner = RawSharedMutex::uninitialized();
        println!("INITIALIZE MUTEX");
        unsafe { me.inner.init(); }

        let u: &mut MaybeUninit<T> =  unsafe { std::mem::transmute(me.data.get_mut()) };
        Init::initialize(u);
    }
}

impl <T: Default> Default for SharedMutex<T> {
    fn default() -> Self {
        Self {
            inner: RawSharedMutex::uninitialized(),
            data: UnsafeCell::new(T::default()),
        }
    }
}

impl<T> SharedMutex<T> {

    pub fn lock(&self) -> SharedMutexGuard<'_, T> {

        unsafe {
            self.inner.lock();
            SharedMutexGuard::new(self)
        }
    }

    pub fn try_lock(&self) -> Option<SharedMutexGuard<'_, T>> {
        unsafe {
            if self.inner.try_lock() {
                Some(SharedMutexGuard::new(self))
            } else {
                None
            }
        }
    }

    pub fn unlock(guard: SharedMutexGuard<'_, T>) {
        drop(guard);
    }

}

pub struct SharedMutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a SharedMutex<T>,

    _phantom_data: PhantomData<*const ()>, // make it !Send
}

unsafe impl<T: ?Sized + Sync> Sync for SharedMutexGuard<'_, T> {}

impl<'a, T: ?Sized> SharedMutexGuard<'a, T> {
    fn new(lock: &'a SharedMutex<T>) -> SharedMutexGuard<'a, T> {
        SharedMutexGuard {
            lock,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: ?Sized> Deref for SharedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T: ?Sized> DerefMut for SharedMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T: ?Sized> Drop for SharedMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.inner.unlock();
        }
    }
}
