#![allow(clippy::all)]

use bevy::{
    ecs::{
        component::{ComponentId, TableStorage},
        query::{Fetch, FetchState, ReadOnlyWorldQuery, WorldQuery, WorldQueryGats},
    },
    prelude::*,
    ptr::{Ptr, PtrMut, ThinSlicePtr},
};

pub trait DynQuery: 'static {}

pub trait DynQueryMarker<Trait: ?Sized + DynQuery> {
    type Covered: Component<Storage = TableStorage>;
    /// Casts an untyped pointer to a trait object pointer,
    /// with a vtable corresponding to `Self::Covered`.
    fn cast(_: *mut u8) -> *mut Trait;
}

pub trait RegisterExt {
    fn register_component_as<Trait: ?Sized + DynQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): DynQueryMarker<Trait, Covered = C>;
}

impl RegisterExt for World {
    fn register_component_as<Trait: ?Sized + DynQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): DynQueryMarker<Trait, Covered = C>,
    {
        let component_id = self.init_component::<C>();
        let registry = self
            .get_resource_or_insert_with(|| TraitComponentRegistry::<Trait> {
                components: vec![],
                meta: vec![],
            })
            .into_inner();
        registry.components.push(component_id);
        registry.meta.push(TraitImplMeta {
            size_bytes: std::mem::size_of::<C>(),
            dyn_ctor: DynCtor { cast: <(C,)>::cast },
        });
        self
    }
}

impl RegisterExt for App {
    fn register_component_as<Trait: ?Sized + DynQuery, C: Component>(&mut self) -> &mut Self
    where
        (C,): DynQueryMarker<Trait, Covered = C>,
    {
        self.world.register_component_as::<Trait, C>();
        self
    }
}

struct TraitComponentRegistry<Trait: ?Sized + DynQuery> {
    // Component IDs are stored contiguously so that we can search them quickly.
    components: Vec<ComponentId>,
    meta: Vec<TraitImplMeta<Trait>>,
}

/// Stores data about an impl of a trait
struct TraitImplMeta<Trait: ?Sized> {
    size_bytes: usize,
    dyn_ctor: DynCtor<Trait>,
}

impl<T: ?Sized> Copy for TraitImplMeta<T> {}
impl<T: ?Sized> Clone for TraitImplMeta<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[doc(hidden)]
pub mod imports {
    pub use bevy::ecs::{
        component::{Component, TableStorage},
        query::{ReadOnlyWorldQuery, WorldQuery, WorldQueryGats},
    };
    pub use bevy::ptr::{Ptr, PtrMut};
}

#[macro_export]
macro_rules! impl_dyn_query {
    ($trait:ident) => {
        impl $crate::DynQuery for dyn $trait {}

        impl<T: $trait + $crate::imports::Component<Storage = $crate::imports::TableStorage>>
            $crate::DynQueryMarker<dyn $trait> for (T,)
        {
            type Covered = T;
            fn cast(ptr: *mut u8) -> *mut dyn $trait {
                ptr as *mut T as *mut _
            }
        }

        unsafe impl<'w> $crate::imports::WorldQuery for &'w dyn $trait {
            type ReadOnly = Self;
            type State = $crate::DynQueryState<dyn $trait>;

            fn shrink<'wlong: 'wshort, 'wshort>(
                item: bevy::ecs::query::QueryItem<'wlong, Self>,
            ) -> bevy::ecs::query::QueryItem<'wshort, Self> {
                item
            }
        }

        unsafe impl $crate::imports::ReadOnlyWorldQuery for &dyn $trait {}

        impl<'w> $crate::imports::WorldQueryGats<'w> for &dyn $trait {
            type Fetch = $crate::ReadTraitComponentsFetch<'w, dyn $trait>;
            type _State = $crate::DynQueryState<dyn $trait>;
        }

        unsafe impl<'w> $crate::imports::WorldQuery for &'w mut dyn $trait {
            type ReadOnly = &'w dyn $trait;
            type State = $crate::DynQueryState<dyn $trait>;

            fn shrink<'wlong: 'wshort, 'wshort>(
                item: bevy::ecs::query::QueryItem<'wlong, Self>,
            ) -> bevy::ecs::query::QueryItem<'wshort, Self> {
                item
            }
        }

        impl<'w> $crate::imports::WorldQueryGats<'w> for &mut dyn $trait {
            type Fetch = $crate::WriteTraitComponentsFetch<'w, dyn $trait>;
            type _State = $crate::DynQueryState<dyn $trait>;
        }
    };
}

#[doc(hidden)]
pub struct DynQueryState<Trait: ?Sized> {
    components: Box<[ComponentId]>,
    meta: Box<[TraitImplMeta<Trait>]>,
}

impl<Trait: ?Sized + DynQuery> FetchState for DynQueryState<Trait> {
    fn init(world: &mut World) -> Self {
        #[cold]
        fn error<T: ?Sized + 'static>() -> ! {
            panic!(
                "no components found matching `{}`, did you forget to register them?",
                std::any::type_name::<T>()
            )
        }

        let registry = world
            .get_resource::<TraitComponentRegistry<Trait>>()
            .unwrap_or_else(|| error::<Trait>());
        Self {
            components: registry.components.clone().into_boxed_slice(),
            meta: registry.meta.clone().into_boxed_slice(),
        }
    }
    fn matches_component_set(&self, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
        self.components.iter().copied().any(set_contains_id)
    }
}

pub struct ReadTraitComponentsFetch<'w, Trait: ?Sized + DynQuery> {
    table_components: Option<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    size_bytes: usize,
    dyn_ctor: Option<DynCtor<Trait>>,
}

/// Turns an untyped pointer into a trait object pointer,
/// for a specific erased concrete type.
struct DynCtor<Trait: ?Sized> {
    cast: unsafe fn(*mut u8) -> *mut Trait,
}

impl<T: ?Sized> Copy for DynCtor<T> {}
impl<T: ?Sized> Clone for DynCtor<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Trait: ?Sized> DynCtor<Trait> {
    unsafe fn cast(self, ptr: Ptr) -> &Trait {
        &*(self.cast)(ptr.as_ptr())
    }
    unsafe fn cast_mut(self, ptr: PtrMut) -> &mut Trait {
        &mut *(self.cast)(ptr.as_ptr())
    }
}

unsafe impl<'w, Trait: ?Sized + DynQuery> Fetch<'w> for ReadTraitComponentsFetch<'w, Trait> {
    type Item = &'w Trait;
    type State = DynQueryState<Trait>;

    unsafe fn init(
        _world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            table_components: None,
            entity_table_rows: None,
            size_bytes: 0,
            dyn_ctor: None,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        tables: &'w bevy::ecs::storage::Tables,
    ) {
        self.entity_table_rows = Some(archetype.entity_table_rows().into());
        let table = &tables[archetype.table_id()];
        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.size_bytes = meta.size_bytes;
                self.dyn_ctor = Some(meta.dyn_ctor);
                return;
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        let ((entity_table_rows, table_components), dyn_ctor) = self
            .entity_table_rows
            .zip(self.table_components)
            .zip(self.dyn_ctor)
            .unwrap_or_else(|| debug_unreachable());
        let table_row = *entity_table_rows.get(archetype_index);
        let ptr = table_components.byte_add(table_row * self.size_bytes);
        dyn_ctor.cast(ptr)
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &'w bevy::ecs::storage::Table) {
        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.size_bytes = meta.size_bytes;
                self.dyn_ctor = Some(meta.dyn_ctor);
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        let (table_components, dyn_ctor) = self
            .table_components
            .zip(self.dyn_ctor)
            .unwrap_or_else(|| debug_unreachable());
        let ptr = table_components.byte_add(table_row * self.size_bytes);
        dyn_ctor.cast(ptr)
    }

    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                    std::any::type_name::<Trait>(),
            );
            access.add_read(component);
        }
    }

    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_read(archetype_component_id);
            }
        }
    }
}

pub struct WriteTraitComponentsFetch<'w, Trait: ?Sized + DynQuery> {
    table_components: Option<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    size_bytes: usize,
    dyn_ctor: Option<DynCtor<Trait>>,
}

unsafe impl<'w, Trait: ?Sized + DynQuery> Fetch<'w> for WriteTraitComponentsFetch<'w, Trait> {
    type Item = &'w mut Trait;
    type State = DynQueryState<Trait>;

    unsafe fn init(
        _world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            table_components: None,
            entity_table_rows: None,
            size_bytes: 0,
            dyn_ctor: None,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = true;

    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        tables: &'w bevy::ecs::storage::Tables,
    ) {
        self.entity_table_rows = Some(archetype.entity_table_rows().into());
        let table = &tables[archetype.table_id()];
        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.size_bytes = meta.size_bytes;
                self.dyn_ctor = Some(meta.dyn_ctor);
                return;
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        let ((entity_table_rows, table_components), dyn_ctor) = self
            .entity_table_rows
            .zip(self.table_components)
            .zip(self.dyn_ctor)
            .unwrap_or_else(|| debug_unreachable());
        let table_row = *entity_table_rows.get(archetype_index);
        let ptr = table_components.byte_add(table_row * self.size_bytes);
        // Is `assert_unique` correct here??
        dyn_ctor.cast_mut(ptr.assert_unique())
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &'w bevy::ecs::storage::Table) {
        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.size_bytes = meta.size_bytes;
                self.dyn_ctor = Some(meta.dyn_ctor);
                return;
            }
        }
        // At least one of the components must be present in the table.
        debug_unreachable()
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        let (table_components, dyn_ctor) = self
            .table_components
            .zip(self.dyn_ctor)
            .unwrap_or_else(|| debug_unreachable());
        let ptr = table_components.byte_add(table_row * self.size_bytes);
        // Is `assert_unique` correct here??
        dyn_ctor.cast_mut(ptr.assert_unique())
    }

    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&mut {} conflicts with a previous access in this query. Mutable component access must be unique.",
                    std::any::type_name::<Trait>(),
            );
            access.add_write(component);
        }
    }

    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_write(archetype_component_id);
            }
        }
    }
}

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity.
pub struct All<T: ?Sized>(T);

#[doc(hidden)]
pub struct ReadAllTraitComponentsFetch<'w, Trait: ?Sized> {
    table_components: Vec<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    size_bytes: Vec<usize>,
    dyn_ctors: Vec<DynCtor<Trait>>,
}

pub struct ReadTraits<'w, Trait: ?Sized> {
    trait_objects: Vec<&'w Trait>,
}

#[doc(hidden)]
pub struct ReadTraitsIter<'world, 'local, Trait: ?Sized> {
    inner: std::slice::Iter<'local, &'world Trait>,
}

impl<'w, 'l, Trait: ?Sized> Iterator for ReadTraitsIter<'w, 'l, Trait> {
    type Item = &'w Trait;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().copied()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'w, Trait: ?Sized> IntoIterator for ReadTraits<'w, Trait> {
    type Item = &'w Trait;
    type IntoIter = std::vec::IntoIter<&'w Trait>;
    fn into_iter(self) -> Self::IntoIter {
        self.trait_objects.into_iter()
    }
}
impl<'w, 'a, Trait: ?Sized> IntoIterator for &'a ReadTraits<'w, Trait> {
    type Item = &'w Trait;
    type IntoIter = ReadTraitsIter<'w, 'a, Trait>;
    fn into_iter(self) -> Self::IntoIter {
        ReadTraitsIter {
            inner: self.trait_objects.iter(),
        }
    }
}

pub struct WriteTraits<'w, Trait: ?Sized> {
    trait_objects: Vec<&'w mut Trait>,
}

#[doc(hidden)]
pub struct WriteTraitsIter<'world, 'local, Trait: ?Sized> {
    inner: std::slice::IterMut<'local, &'world mut Trait>,
}

impl<'w, 'l, Trait: ?Sized> Iterator for WriteTraitsIter<'w, 'l, Trait> {
    type Item = &'l mut Trait;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|x| &mut **x)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'w, Trait: ?Sized> IntoIterator for WriteTraits<'w, Trait> {
    type Item = &'w mut Trait;
    type IntoIter = std::vec::IntoIter<&'w mut Trait>;
    fn into_iter(self) -> Self::IntoIter {
        self.trait_objects.into_iter()
    }
}
impl<'w, 'a, Trait: ?Sized> IntoIterator for &'a mut WriteTraits<'w, Trait> {
    type Item = &'a mut Trait;
    type IntoIter = WriteTraitsIter<'w, 'a, Trait>;
    fn into_iter(self) -> Self::IntoIter {
        WriteTraitsIter {
            inner: self.trait_objects.iter_mut(),
        }
    }
}

unsafe impl<'w, Trait: ?Sized + DynQuery> Fetch<'w> for ReadAllTraitComponentsFetch<'w, Trait> {
    type Item = ReadTraits<'w, Trait>;
    type State = DynQueryState<Trait>;

    unsafe fn init(
        _world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            table_components: vec![],
            entity_table_rows: None,
            size_bytes: vec![],
            dyn_ctors: vec![],
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = true;

    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        tables: &'w bevy::ecs::storage::Tables,
    ) {
        self.table_components.clear();
        self.size_bytes.clear();
        self.dyn_ctors.clear();

        self.entity_table_rows = Some(archetype.entity_table_rows().into());
        let table = &tables[archetype.table_id()];
        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components.push(column.get_data_ptr());
                self.size_bytes.push(meta.size_bytes);
                self.dyn_ctors.push(meta.dyn_ctor);
            }
        }
        // At least one component must match.
        assert!(!self.table_components.is_empty());
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        let entity_table_rows = self
            .entity_table_rows
            .unwrap_or_else(|| debug_unreachable());
        let table_row = *entity_table_rows.get(archetype_index);

        let mut trait_objects = Vec::with_capacity(self.table_components.len());
        for ((&table_components, &dyn_ctor), &size_bytes) in
            std::iter::zip(&self.table_components, &self.dyn_ctors).zip(&self.size_bytes)
        {
            let ptr = table_components.byte_add(table_row * size_bytes);
            trait_objects.push(dyn_ctor.cast(ptr));
        }

        ReadTraits { trait_objects }
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &'w bevy::ecs::storage::Table) {
        self.table_components.clear();
        self.size_bytes.clear();
        self.dyn_ctors.clear();

        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components.push(column.get_data_ptr());
                self.size_bytes.push(meta.size_bytes);
                self.dyn_ctors.push(meta.dyn_ctor);
            }
        }
        // At least one component must match.
        assert!(!self.table_components.is_empty());
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        let mut trait_objects = Vec::with_capacity(self.table_components.len());
        for ((&table_components, &dyn_ctor), &size_bytes) in
            std::iter::zip(&self.table_components, &self.dyn_ctors).zip(&self.size_bytes)
        {
            let ptr = table_components.byte_add(table_row * size_bytes);
            trait_objects.push(dyn_ctor.cast(ptr));
        }

        ReadTraits { trait_objects }
    }

    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                    std::any::type_name::<Trait>(),
            );
            access.add_read(component);
        }
    }

    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_read(archetype_component_id);
            }
        }
    }
}

#[doc(hidden)]
pub struct WriteAllTraitComponentsFetch<'w, Trait: ?Sized> {
    table_components: Vec<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    size_bytes: Vec<usize>,
    dyn_ctors: Vec<DynCtor<Trait>>,
}

unsafe impl<'w, Trait: ?Sized + DynQuery> Fetch<'w> for WriteAllTraitComponentsFetch<'w, Trait> {
    type Item = WriteTraits<'w, Trait>;
    type State = DynQueryState<Trait>;

    unsafe fn init(
        _world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            table_components: vec![],
            entity_table_rows: None,
            size_bytes: vec![],
            dyn_ctors: vec![],
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = true;

    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &'w bevy::ecs::archetype::Archetype,
        tables: &'w bevy::ecs::storage::Tables,
    ) {
        self.table_components.clear();
        self.size_bytes.clear();
        self.dyn_ctors.clear();

        self.entity_table_rows = Some(archetype.entity_table_rows().into());
        let table = &tables[archetype.table_id()];
        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components.push(column.get_data_ptr());
                self.size_bytes.push(meta.size_bytes);
                self.dyn_ctors.push(meta.dyn_ctor);
            }
        }
        // At least one component must match.
        assert!(!self.table_components.is_empty());
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        let entity_table_rows = self
            .entity_table_rows
            .unwrap_or_else(|| debug_unreachable());
        let table_row = *entity_table_rows.get(archetype_index);

        let mut trait_objects = Vec::with_capacity(self.table_components.len());
        for ((&table_components, &dyn_ctor), &size_bytes) in
            std::iter::zip(&self.table_components, &self.dyn_ctors).zip(&self.size_bytes)
        {
            let ptr = table_components.byte_add(table_row * size_bytes);
            trait_objects.push(dyn_ctor.cast_mut(ptr.assert_unique()));
        }

        WriteTraits { trait_objects }
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &'w bevy::ecs::storage::Table) {
        self.table_components.clear();
        self.size_bytes.clear();
        self.dyn_ctors.clear();

        for (&component, meta) in std::iter::zip(&*state.components, &*state.meta) {
            if let Some(column) = table.get_column(component) {
                self.table_components.push(column.get_data_ptr());
                self.size_bytes.push(meta.size_bytes);
                self.dyn_ctors.push(meta.dyn_ctor);
            }
        }
        // At least one component must match.
        assert!(!self.table_components.is_empty());
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        let mut trait_objects = Vec::with_capacity(self.table_components.len());
        for ((&table_components, &dyn_ctor), &size_bytes) in
            std::iter::zip(&self.table_components, &self.dyn_ctors).zip(&self.size_bytes)
        {
            let ptr = table_components.byte_add(table_row * size_bytes);
            trait_objects.push(dyn_ctor.cast_mut(ptr.assert_unique()));
        }

        WriteTraits { trait_objects }
    }

    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &*state.components {
            assert!(
                !access.access().has_write(component),
                "&mut {} conflicts with a previous access in this query. Mutable component access must be unique.",
                    std::any::type_name::<Trait>(),
            );
            access.add_write(component);
        }
    }

    fn update_archetype_component_access(
        state: &Self::State,
        archetype: &bevy::ecs::archetype::Archetype,
        access: &mut bevy::ecs::query::Access<bevy::ecs::archetype::ArchetypeComponentId>,
    ) {
        for &component in &*state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_write(archetype_component_id);
            }
        }
    }
}

unsafe impl<'w, Trait: ?Sized + DynQuery> WorldQuery for All<&'w Trait> {
    type ReadOnly = Self;
    type State = DynQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(
        item: bevy::ecs::query::QueryItem<'wlong, Self>,
    ) -> bevy::ecs::query::QueryItem<'wshort, Self> {
        item
    }
}

unsafe impl<Trait: ?Sized + DynQuery> ReadOnlyWorldQuery for All<&Trait> {}

impl<'w, Trait: ?Sized + DynQuery> WorldQueryGats<'w> for All<&Trait> {
    type Fetch = ReadAllTraitComponentsFetch<'w, Trait>;
    type _State = DynQueryState<Trait>;
}

unsafe impl<'w, Trait: ?Sized + DynQuery> WorldQuery for All<&'w mut Trait> {
    type ReadOnly = All<&'w Trait>;
    type State = DynQueryState<Trait>;

    fn shrink<'wlong: 'wshort, 'wshort>(
        item: bevy::ecs::query::QueryItem<'wlong, Self>,
    ) -> bevy::ecs::query::QueryItem<'wshort, Self> {
        item
    }
}

impl<'w, Trait: ?Sized + DynQuery> WorldQueryGats<'w> for All<&mut Trait> {
    type Fetch = WriteAllTraitComponentsFetch<'w, Trait>;
    type _State = DynQueryState<Trait>;
}

#[track_caller]
#[inline(always)]
unsafe fn debug_unreachable() -> ! {
    #[cfg(debug_assertions)]
    unreachable!();

    #[cfg(not(debug_assertions))]
    std::hint::unreachable_unchecked();
}
