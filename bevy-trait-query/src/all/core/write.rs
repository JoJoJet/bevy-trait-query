use bevy_ecs::{
    change_detection::{DetectChanges, Mut, Ref},
    component::{ComponentId, Tick},
    entity::Entity,
    ptr::UnsafeCellDeref,
    storage::{SparseSets, Table, TableRow},
};

use crate::{
    zip_exact, CombinedReadTraitsIter, ReadSparseTraitsIter, ReadTableTraitsIter, TraitImplMeta,
    TraitImplRegistry, TraitQuery,
};

/// Write-access to all components implementing a trait for a given entity.
///
/// This supports change detection and detection for added objects via
///
/// - [`WriteTraits::iter_changed`]
/// - [`WriteTraits::iter_added`]
pub struct WriteTraits<'a, Trait: ?Sized + TraitQuery> {
    // Read-only access to the global trait registry.
    // Since no one outside of the crate can name the registry type,
    // we can be confident that no write accesses will conflict with this.
    pub(crate) registry: &'a TraitImplRegistry<Trait>,

    pub(crate) table: &'a Table,
    pub(crate) table_row: TableRow,

    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,

    /// This grants shared mutable access to all sparse set components,
    /// but in practice we will only modify the components specified in `self.registry`.
    /// The fetch impl registers write-access for all of these components,
    /// guaranteeing us exclusive access at runtime.
    pub(crate) sparse_sets: &'a SparseSets,
}

#[doc(hidden)]
pub type CombinedWriteTraitsIter<'a, Trait> =
    std::iter::Chain<WriteTableTraitsIter<'a, Trait>, WriteSparseTraitsIter<'a, Trait>>;

#[doc(hidden)]
pub struct WriteTableTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    pub(crate) components: std::slice::Iter<'a, ComponentId>,
    pub(crate) meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    pub(crate) table: &'a Table,
    /// SAFETY: Given the same trait type and same archetype,
    /// no two instances of this struct may have the same `table_row`.
    pub(crate) table_row: TableRow,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for WriteTableTraitsIter<'a, Trait> {
    type Item = Mut<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining table components that are registered,
        // until we find one that exists in the table.
        let (column, meta) = unsafe { zip_exact(&mut self.components, &mut self.meta) }
            .find_map(|(&component, meta)| self.table.get_column(component).zip(Some(meta)))?;
        let ptr = unsafe {
            column
                .get_data_ptr()
                .byte_add(self.table_row.as_usize() * meta.size_bytes)
        };
        // SAFETY: The instance of `WriteTraits` that created this iterator
        // has exclusive access to all table components registered with the trait.
        //
        // Since `self.table_row` is guaranteed to be unique, we know that other instances
        // of `WriteTableTraitsIter` will not conflict with this pointer.
        let ptr = unsafe { ptr.assert_unique() };
        let trait_object = unsafe { meta.dyn_ctor.cast_mut(ptr) };
        // SAFETY: We have exclusive access to the component, so by extension
        // we have exclusive access to the corresponding `ComponentTicks`.
        let added = unsafe { column.get_added_tick_unchecked(self.table_row).deref_mut() };
        let changed = unsafe {
            column
                .get_changed_tick_unchecked(self.table_row)
                .deref_mut()
        };
        Some(Mut::new(
            trait_object,
            added,
            changed,
            self.last_run,
            self.this_run,
        ))
    }
}

#[doc(hidden)]
pub struct WriteSparseTraitsIter<'a, Trait: ?Sized> {
    // SAFETY: These two iterators must have equal length.
    pub(crate) components: std::slice::Iter<'a, ComponentId>,
    pub(crate) meta: std::slice::Iter<'a, TraitImplMeta<Trait>>,
    /// SAFETY: Given the same trait type and same archetype,
    /// no two instances of this struct may have the same `entity`.
    pub(crate) entity: Entity,
    pub(crate) sparse_sets: &'a SparseSets,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'a, Trait: ?Sized + TraitQuery> Iterator for WriteSparseTraitsIter<'a, Trait> {
    type Item = Mut<'a, Trait>;
    fn next(&mut self) -> Option<Self::Item> {
        // Iterate the remaining sparse set components we have registered,
        // until we find one that exists in the archetype.
        let ((ptr, component_ticks), meta) =
            unsafe { zip_exact(&mut self.components, &mut self.meta) }.find_map(
                |(&component, meta)| {
                    self.sparse_sets
                        .get(component)
                        .and_then(|set| set.get_with_ticks(self.entity))
                        .zip(Some(meta))
                },
            )?;

        // SAFETY: The instance of `WriteTraits` that created this iterator
        // has exclusive access to all sparse set components registered with the trait.
        //
        // Since `self.entity` is guaranteed to be unique, we know that other instances
        // of `WriteSparseTraitsIter` will not conflict with this pointer.
        let ptr = unsafe { ptr.assert_unique() };
        let trait_object = unsafe { meta.dyn_ctor.cast_mut(ptr) };
        // SAFETY: We have exclusive access to the component, so by extension
        // we have exclusive access to the corresponding `ComponentTicks`.
        let added = unsafe { component_ticks.added.deref_mut() };
        let changed = unsafe { component_ticks.changed.deref_mut() };

        Some(Mut::new(
            trait_object,
            added,
            changed,
            self.last_run,
            self.this_run,
        ))
    }
}

impl<Trait: ?Sized + TraitQuery> WriteTraits<'_, Trait> {
    /// Returns an iterator over the components implementing `Trait` for the current entity.
    pub fn iter(&self) -> CombinedReadTraitsIter<'_, Trait> {
        self.into_iter()
    }

    /// Returns a mutable iterator over the components implementing `Trait` for the current entity.
    pub fn iter_mut(&mut self) -> CombinedWriteTraitsIter<'_, Trait> {
        self.into_iter()
    }

    /// Returns an iterator over the components implementing `Trait` for the current entity
    /// that were added since the last time the system was run.
    pub fn iter_added(&self) -> impl Iterator<Item = Ref<'_, Trait>> {
        self.iter().filter(DetectChanges::is_added)
    }

    /// Returns an iterator over the components implementing `Trait` for the current entity
    /// whose values were changed since the last time the system was run.
    pub fn iter_changed(&self) -> impl Iterator<Item = Ref<'_, Trait>> {
        self.iter().filter(DetectChanges::is_changed)
    }

    /// Returns a mutable iterator over the components implementing `Trait` for the current entity
    /// that were added since the last time the system was run.
    pub fn iter_added_mut(&mut self) -> impl Iterator<Item = Mut<'_, Trait>> {
        self.iter_mut().filter(DetectChanges::is_added)
    }

    /// Returns a mutable iterator over the components implementing `Trait` for the current entity
    /// whose values were changed since the last time the system was run.
    pub fn iter_changed_mut(&mut self) -> impl Iterator<Item = Mut<'_, Trait>> {
        self.iter_mut().filter(DetectChanges::is_changed)
    }
}

impl<'w, Trait: ?Sized + TraitQuery> IntoIterator for WriteTraits<'w, Trait> {
    type Item = Mut<'w, Trait>;
    type IntoIter = CombinedWriteTraitsIter<'w, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = WriteTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = WriteSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.as_usize()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local WriteTraits<'world, Trait>
{
    type Item = Ref<'local, Trait>;
    type IntoIter = CombinedReadTraitsIter<'local, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = ReadTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = ReadSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.as_usize()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}

impl<'world, 'local, Trait: ?Sized + TraitQuery> IntoIterator
    for &'local mut WriteTraits<'world, Trait>
{
    type Item = Mut<'local, Trait>;
    type IntoIter = CombinedWriteTraitsIter<'local, Trait>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let table = WriteTableTraitsIter {
            components: self.registry.table_components.iter(),
            meta: self.registry.table_meta.iter(),
            table: self.table,
            table_row: self.table_row,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        let sparse = WriteSparseTraitsIter {
            components: self.registry.sparse_components.iter(),
            meta: self.registry.sparse_meta.iter(),
            entity: self.table.entities()[self.table_row.as_usize()],
            sparse_sets: self.sparse_sets,
            last_run: self.last_run,
            this_run: self.this_run,
        };
        table.chain(sparse)
    }
}
