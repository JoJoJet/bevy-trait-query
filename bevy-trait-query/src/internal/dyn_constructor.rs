use bevy_ecs::ptr::{Ptr, PtrMut};

/// Turns an untyped pointer into a trait object pointer,
/// for a specific erased concrete type.
pub(crate) struct DynCtor<Trait: ?Sized> {
    pub(crate) cast: unsafe fn(*mut u8) -> *mut Trait,
}

impl<T: ?Sized> Copy for DynCtor<T> {}
impl<T: ?Sized> Clone for DynCtor<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Trait: ?Sized> DynCtor<Trait> {
    #[inline]
    pub(crate) unsafe fn cast(self, ptr: Ptr) -> &Trait {
        &*(self.cast)(ptr.as_ptr())
    }
    #[inline]
    pub(crate) unsafe fn cast_mut(self, ptr: PtrMut) -> &mut Trait {
        &mut *(self.cast)(ptr.as_ptr())
    }
}
