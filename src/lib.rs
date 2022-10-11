#![allow(clippy::all)]

use std::marker::PhantomData;

use bevy::{
    ecs::{
        component::{ComponentId, TableStorage},
        query::{Fetch, FetchState, ReadOnlyWorldQuery, WorldQuery, WorldQueryGats},
    },
    prelude::*,
    ptr::{Ptr, PtrMut, ThinSlicePtr},
};

pub trait DynQuery: 'static {}

pub trait ComponentWithTrait<Dyn: ?Sized + 'static>: Component<Storage = TableStorage> {
    unsafe fn get_dyn(_: Ptr, index: usize) -> &Dyn;
    unsafe fn get_dyn_mut(_: PtrMut, index: usize) -> &mut Dyn;
}

pub trait RegisterExt {
    fn register_component_as<Trait: ?Sized + DynQuery, C: ComponentWithTrait<Trait>>(
        &mut self,
    ) -> &mut Self;
}

impl RegisterExt for World {
    fn register_component_as<Trait: ?Sized + DynQuery, C: ComponentWithTrait<Trait>>(
        &mut self,
    ) -> &mut Self {
        let component_id = self.init_component::<C>();
        let registry = self
            .get_resource_or_insert_with(|| TraitComponentRegistry::<Trait> {
                components: vec![],
                cast_dyn: vec![],
                cast_dyn_mut: vec![],
                marker: PhantomData,
            })
            .into_inner();
        registry.components.push(component_id);
        registry.cast_dyn.push(C::get_dyn);
        registry.cast_dyn_mut.push(C::get_dyn_mut);
        self
    }
}

impl RegisterExt for App {
    fn register_component_as<Trait: ?Sized + DynQuery, C: ComponentWithTrait<Trait>>(
        &mut self,
    ) -> &mut Self {
        self.world.register_component_as::<Trait, C>();
        self
    }
}

pub struct TraitComponentRegistry<Dyn: ?Sized + DynQuery> {
    components: Vec<ComponentId>,
    cast_dyn: Vec<unsafe fn(Ptr, usize) -> &Dyn>,
    cast_dyn_mut: Vec<unsafe fn(PtrMut, usize) -> &mut Dyn>,
    marker: PhantomData<fn() -> Dyn>,
}

impl<T: ?Sized + DynQuery> Clone for TraitComponentRegistry<T> {
    fn clone(&self) -> Self {
        Self {
            components: self.components.clone(),
            cast_dyn: self.cast_dyn.clone(),
            cast_dyn_mut: self.cast_dyn_mut.clone(),
            marker: PhantomData,
        }
    }
}

pub trait Foo: 'static {
    fn name(&self) -> &str;
    fn age(&self) -> u32;
    fn set_age(&mut self, age: u32);
}

#[macro_export]
macro_rules! impl_dyn_query {
    ($trait:ident) => {
        impl $crate::DynQuery for dyn $trait {}

        impl<T: $trait + Component<Storage = TableStorage>> ComponentWithTrait<dyn $trait> for T {
            unsafe fn get_dyn(ptr: Ptr, index: usize) -> &dyn $trait {
                let offset = (index * std::mem::size_of::<Self>()) as isize;
                ptr.byte_offset(offset).deref::<Self>()
            }
            unsafe fn get_dyn_mut(ptr: PtrMut, index: usize) -> &mut dyn $trait {
                let offset = (index * std::mem::size_of::<Self>()) as isize;
                ptr.byte_offset(offset).deref_mut::<Self>()
            }
        }

        unsafe impl<'w> WorldQuery for &'w dyn $trait {
            type ReadOnly = Self;
            type State = TraitComponentRegistry<dyn $trait>;

            fn shrink<'wlong: 'wshort, 'wshort>(
                item: bevy::ecs::query::QueryItem<'wlong, Self>,
            ) -> bevy::ecs::query::QueryItem<'wshort, Self> {
                item
            }
        }

        unsafe impl ReadOnlyWorldQuery for &dyn $trait {}

        impl<'w> WorldQueryGats<'w> for &dyn $trait {
            type Fetch = ReadTraitComponentsFetch<'w, dyn $trait>;
            type _State = TraitComponentRegistry<dyn $trait>;
        }

        unsafe impl<'w> WorldQuery for &'w mut dyn $trait {
            type ReadOnly = &'w dyn $trait;
            type State = TraitComponentRegistry<dyn $trait>;

            fn shrink<'wlong: 'wshort, 'wshort>(
                item: bevy::ecs::query::QueryItem<'wlong, Self>,
            ) -> bevy::ecs::query::QueryItem<'wshort, Self> {
                item
            }
        }

        impl<'w> WorldQueryGats<'w> for &mut dyn $trait {
            type Fetch = WriteTraitComponentsFetch<'w, dyn $trait>;
            type _State = TraitComponentRegistry<dyn $trait>;
        }
    };
}

impl_dyn_query!(Foo);

impl<Trait: ?Sized + DynQuery> FetchState for TraitComponentRegistry<Trait> {
    fn init(world: &mut World) -> Self {
        #[cold]
        fn error<T: ?Sized + 'static>() -> ! {
            panic!(
                "no components found matching `{}`, did you forget to register them?",
                std::any::type_name::<T>()
            )
        }

        world
            .get_resource::<TraitComponentRegistry<Trait>>()
            .unwrap_or_else(|| error::<Trait>())
            .clone()
    }
    fn matches_component_set(&self, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
        self.components.iter().copied().any(set_contains_id)
    }
}

pub struct ReadTraitComponentsFetch<'w, Trait: ?Sized + DynQuery> {
    table_components: Option<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    cast_dyn: Option<unsafe fn(Ptr, usize) -> &Trait>,
}

unsafe impl<'w, Trait: ?Sized + DynQuery> Fetch<'w> for ReadTraitComponentsFetch<'w, Trait> {
    type Item = &'w Trait;
    type State = TraitComponentRegistry<Trait>;

    unsafe fn init(
        _world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            table_components: None,
            entity_table_rows: None,
            cast_dyn: None,
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
        for (&component, &cast) in std::iter::zip(&state.components, &state.cast_dyn) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.cast_dyn = Some(cast);
                return;
            }
        }
        // At least one of the components must be present in the table.
        unreachable!()
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        let ((entity_table_rows, table_components), cast_dyn) = self
            .entity_table_rows
            .zip(self.table_components)
            .zip(self.cast_dyn)
            .unwrap();
        let table_row = *entity_table_rows.get(archetype_index);
        cast_dyn(table_components, table_row)
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &'w bevy::ecs::storage::Table) {
        for (&component, &cast) in std::iter::zip(&state.components, &state.cast_dyn) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.cast_dyn = Some(cast);
                return;
            }
        }
        // At least one of the components must be present in the table.
        unreachable!();
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        let (table_components, cast_dyn) = self.table_components.zip(self.cast_dyn).unwrap();
        cast_dyn(table_components, table_row)
    }

    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &state.components {
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
        for &component in &state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_read(archetype_component_id);
            }
        }
    }
}

pub struct WriteTraitComponentsFetch<'w, Trait: ?Sized + DynQuery> {
    table_components: Option<Ptr<'w>>,
    entity_table_rows: Option<ThinSlicePtr<'w, usize>>,
    cast_dyn: Option<unsafe fn(PtrMut, usize) -> &mut Trait>,
}

unsafe impl<'w, Trait: ?Sized + DynQuery> Fetch<'w> for WriteTraitComponentsFetch<'w, Trait> {
    type Item = &'w mut Trait;
    type State = TraitComponentRegistry<Trait>;

    unsafe fn init(
        _world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            table_components: None,
            entity_table_rows: None,
            cast_dyn: None,
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
        for (&component, &cast) in std::iter::zip(&state.components, &state.cast_dyn_mut) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.cast_dyn = Some(cast);
                return;
            }
        }
        // At least one of the components must be present in the table.
        unreachable!()
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        let ((entity_table_rows, table_components), cast_dyn) = self
            .entity_table_rows
            .zip(self.table_components)
            .zip(self.cast_dyn)
            .unwrap();
        let table_row = *entity_table_rows.get(archetype_index);
        // Is `assert_unique` correct here??
        cast_dyn(table_components.assert_unique(), table_row)
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &'w bevy::ecs::storage::Table) {
        for (&component, &cast) in std::iter::zip(&state.components, &state.cast_dyn_mut) {
            if let Some(column) = table.get_column(component) {
                self.table_components = Some(column.get_data_ptr());
                self.cast_dyn = Some(cast);
                return;
            }
        }
        // At least one of the components must be present in the table.
        unreachable!();
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        let (table_components, cast_dyn) = self.table_components.zip(self.cast_dyn).unwrap();
        // Is `assert_unique` correct here??
        cast_dyn(table_components.assert_unique(), table_row)
    }

    fn update_component_access(
        state: &Self::State,
        access: &mut bevy::ecs::query::FilteredAccess<ComponentId>,
    ) {
        for &component in &state.components {
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
        for &component in &state.components {
            if let Some(archetype_component_id) = archetype.get_archetype_component_id(component) {
                access.add_write(archetype_component_id);
            }
        }
    }
}
