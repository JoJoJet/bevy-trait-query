# Changelog

## Version 0.7 (Bevy 0.15)

* Added support for Bevy 0.15.

## Version 0.6 (Bevy 0.14)

* Added support for Bevy 0.14.
* Added `WithoutAny` filter to check if an entity is not holding any component of a certain trait
* Update `WithOne` to only implement `QueryFilter`. This means it's not usable in the data position anymore. To migrate this change, use `One` instead, which is the intended way of doing that

## Version 0.3 (Bevy 0.11)

* Added support for Bevy 0.11.
* Updated the `syn` crate to version 2.0.
* Trait queries now use bevy's built-in `Mut<T>` type for change detection;
this crate's reimplementation of this type has been removed.

## Version 0.2.1

### Fixed

* Fixed change detection reporting for trait queries.

## Version 0.2 (Bevy 0.10)

* Added support for Bevy 0.10.

## Version 0.1.1

### Fixed

* Fixed hygiene for the `#[queryable]` macro.

## Version 0.1 (Bevy 0.9)

### Added

* Added the `#[queryable]` macro, which lets you add query support to a trait declaration.
* Added support for generic traits.
* Added `&dyn Trait` and `&mut dyn Trait` as queries, which use the `All<>` behavior.
* Added iterator methods to `All<>` query items.

### Changed

* Instead of panicking, a warning is now emitted when no trait impls are registered.
* Bevy's default features are no longer required.

### Removed

* Removed the `impl_trait_query` declarative macro.

## Version 0.0.3 (Bevy 0.8)

### Fixed

* Duplicate trait impl registrations are now ignored (soundness issue).

## Version 0.0.2 (Bevy 0.8)

### Added

* Added the `One<>` adapter for trait queries.

### Removed

* `&dyn Trait` and `&mut dyn Trait` can no longer be used as a `WorldQuery` directly
-- you must explicitly choose between `One<>` and `All<>`.

## Version 0.0.1 (Bevy 0.8)

* Initial release.
