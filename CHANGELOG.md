# Changelog

## Version 0.1 (Bevy 0.9)

### Added

* Added the `#[queryable]` macro, which adds query support to a trait definition.
* Added `&dyn Trait` and `&mut dyn Trait` as queries, which use the `All<>` behavior.
* Added iterator methods to `All<>` query items.

### Changed

* Instead of panicking, a warning is now emitted when no trait impls are registered.
* Bevy's default features are no longer enabled.

### Removed

* Removed the `impl_trait_query` declarative macro.

## Version 0.0.3 (Bevy 0.8)

### Fixed

* Duplicate trait impl registrations are now ignored (soundness issue).

## Version 0.0.2 (Bevy 0.8)

* Added the `One<>` adapter for trait queries.
* `&dyn Trait` and `&mut dyn Trait` can no longer be used as a `WorldQuery` directly
-- you must explicitly choose between `One<>` and `All<>`.

## Version 0.0.1 (Bevy 0.8)

* Initial release.
