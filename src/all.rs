use crate::multiple_read::{ReadAllTraitsFetch, ReadTraits};
use crate::{debug_unreachable, TraitQuery, TraitQueryState, WriteAllTraitsFetch, WriteTraits};
use bevy::ecs::component::ComponentId;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryItem, ReadOnlyWorldQuery, WorldQuery};
use bevy::ecs::world::World;

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity.
///
/// You can usually just use `&dyn Trait` or `&mut dyn Trait` as a `WorldQuery` directly.
pub struct All<T: ?Sized>(T);

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity, with
/// the additional condition that they have also changed since the last tick.
pub struct ChangedAll<T: ?Sized>(T);

/// `WorldQuery` adapter that fetches all implementations of a given trait for an entity, with
/// the additional condition that they have been added since the last tick.
pub struct AddedAll<T: ?Sized>(T);

unsafe impl<'a, Trait: ?Sized + TraitQuery> ReadOnlyWorldQuery for All<&'a Trait> {}

/// SAFETY: We only access the components registered in the trait registry.
/// This is known to match the set of components in the `DynQueryState`,
/// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for All<&'a Trait> {
    type Item<'w> = ReadTraits<'w, Trait>;
    type Fetch<'w> = ReadAllTraitsFetch<'w, Trait>;
    type ReadOnly = Self;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: &'w World,
        _state: &Self::State,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> ReadAllTraitsFetch<'w, Trait> {
        ReadAllTraitsFetch {
            registry: world.resource(),
            table: None,
            sparse_sets: &world.storages().sparse_sets,
        }
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        ReadAllTraitsFetch {
            registry: fetch.registry,
            table: fetch.table,
            sparse_sets: fetch.sparse_sets,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut ReadAllTraitsFetch<'w, Trait>,
        _state: &Self::State,
        _archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    unsafe fn set_table<'w>(
        fetch: &mut ReadAllTraitsFetch<'w, Trait>,
        _state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        _entity: Entity,
        table_row: usize,
    ) -> Self::Item<'w> {
        let table = fetch.table.unwrap_or_else(|| debug_unreachable());

        ReadTraits {
            registry: fetch.registry,
            table,
            table_row,
            sparse_sets: fetch.sparse_sets,
        }
    }

    #[inline]
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

    #[inline]
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

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_any(set_contains_id)
    }
}

/// SAFETY: We only access the components registered in the trait registry.
/// This is known to match the set of components in the `DynQueryState`,
/// which is used to match archetypes and register world access.
unsafe impl<'a, Trait: ?Sized + TraitQuery> WorldQuery for All<&'a mut Trait> {
    type Item<'w> = WriteTraits<'w, Trait>;
    type Fetch<'w> = WriteAllTraitsFetch<'w, Trait>;
    type ReadOnly = All<&'a Trait>;
    type State = TraitQueryState<Trait>;

    #[inline]
    fn shrink<'wlong: 'wshort, 'wshort>(item: QueryItem<'wlong, Self>) -> QueryItem<'wshort, Self> {
        item
    }

    #[inline]
    unsafe fn init_fetch<'w>(
        world: &'w World,
        _state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> WriteAllTraitsFetch<'w, Trait> {
        WriteAllTraitsFetch {
            registry: world.resource(),
            table: None,
            sparse_sets: &world.storages().sparse_sets,
            last_change_tick,
            change_tick,
        }
    }

    #[inline]
    unsafe fn clone_fetch<'w>(fetch: &Self::Fetch<'w>) -> Self::Fetch<'w> {
        WriteAllTraitsFetch {
            registry: fetch.registry,
            table: fetch.table,
            sparse_sets: fetch.sparse_sets,
            last_change_tick: fetch.last_change_tick,
            change_tick: fetch.change_tick,
        }
    }

    const IS_DENSE: bool = false;
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn set_archetype<'w>(
        fetch: &mut WriteAllTraitsFetch<'w, Trait>,
        _state: &Self::State,
        _archetype: &'w bevy::ecs::archetype::Archetype,
        table: &'w bevy::ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn set_table<'w>(
        fetch: &mut WriteAllTraitsFetch<'w, Trait>,
        _state: &Self::State,
        table: &'w bevy::ecs::storage::Table,
    ) {
        fetch.table = Some(table);
    }

    #[inline]
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        _entity: Entity,
        table_row: usize,
    ) -> Self::Item<'w> {
        let table = fetch.table.unwrap_or_else(|| debug_unreachable());

        WriteTraits {
            registry: fetch.registry,
            table,
            table_row,
            sparse_sets: fetch.sparse_sets,
            last_change_tick: fetch.last_change_tick,
            change_tick: fetch.change_tick,
        }
    }

    #[inline]
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

    #[inline]
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

    #[inline]
    fn init_state(world: &mut World) -> Self::State {
        TraitQueryState::init(world)
    }
    #[inline]
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        state.matches_component_set_any(set_contains_id)
    }
}
