//! Helpers for borrowing and self-borrowing values.

use std::mem::ManuallyDrop;

/// This ties two values together, so that one value can borrow from the other, while allowing the
/// resulting object to be stored in a struct. The life time of the borrow will not infect the
/// surrounding type's signature.
///
/// A `Tied` value dereferences to its produced borrowing value, and can likely be used as a
/// drop-in replacement for existing code which needs to get rid of lifetimes.
///
/// Example:
/// ```
/// struct Owner(i64);
/// struct Borrow<'a>(&'a mut i64);
///
/// impl Owner {
///     pub fn borrow_mut(&mut self) -> Borrow {
///         Borrow(&mut self.0)
///     }
/// }
///
/// // Show that we can be used as a Borrow
/// impl<'a> Borrow<'a> {
///     pub fn i_am_a_borrow(&self) {}
/// }
///
/// // The following cannot be expressed in rust:
/// //struct Usage {
/// //    owner: Owner,
/// //    borrow: Borrow<??? lifetime of self.owner ???>
/// //}
///
/// // Instead we use:
/// use proxmox::tools::borrow::Tied;
/// struct Usage {
///     tied: Tied<Owner, Borrow<'static>>,
/// }
///
/// let usage = Usage {
///     tied: Tied::new(Owner(10), |owner| Box::new(unsafe { (*owner).borrow_mut() })),
/// };
///
/// // tied can be used like a Borrow:
/// usage.tied.i_am_a_borrow();
/// ```
pub struct Tied<T, U: ?Sized> {
    // FIXME: ManuallyDrop::take() is nightly-only so we need an Option for inner for now...
    /// The contained "value" of which we want to borrow something.
    inner: Option<Box<T>>,
    /// The thing borrowing from `inner`. This is what the `Tied` value ultimately dereferences to.
    borrow: ManuallyDrop<Box<U>>,
}

impl<T, U: ?Sized> Drop for Tied<T, U> {
    fn drop(&mut self) {
        unsafe {
            // let's be explicit about order here!
            ManuallyDrop::drop(&mut self.borrow);
            let _ = self.inner.take();
            //ManuallyDrop::drop(&mut self.inner);
        }
    }
}

impl<T, U: ?Sized> Tied<T, U> {
    /// Takes a value and a function producing the borrowing value. The owning value will be
    /// inaccessible until the tied value is resolved. The dependent value is only accessible by
    /// reference.
    pub fn new<F>(value: T, producer: F) -> Self
    where
        F: FnOnce(*mut T) -> Box<U>,
    {
        let mut value = Box::new(value);
        let borrow = producer(&mut *value);
        Self {
            inner: Some(value),
            borrow: ManuallyDrop::new(borrow),
        }
    }

    pub fn into_boxed_inner(mut self) -> Box<T> {
        unsafe {
            ManuallyDrop::drop(&mut self.borrow);
            //ManuallyDrop::take(&mut self.inner)
        }
        self.inner.take().unwrap()
    }

    pub fn into_inner(self) -> T {
        *self.into_boxed_inner()
    }
}

impl<T, U: ?Sized> AsRef<U> for Tied<T, U> {
    fn as_ref(&self) -> &U {
        &self.borrow
    }
}

impl<T, U: ?Sized> AsMut<U> for Tied<T, U> {
    fn as_mut(&mut self) -> &mut U {
        &mut self.borrow
    }
}

impl<T, U: ?Sized> std::ops::Deref for Tied<T, U> {
    type Target = U;

    fn deref(&self) -> &U {
        self.as_ref()
    }
}

impl<T, U: ?Sized> std::ops::DerefMut for Tied<T, U> {
    fn deref_mut(&mut self) -> &mut U {
        self.as_mut()
    }
}
