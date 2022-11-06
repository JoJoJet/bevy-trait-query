# Changelog

## Version 0.1

## Version 0.0.3

### Fixed

* Duplicate trait impl registrations are now ignored (soundness issue).

## Version 0.0.2

* Added the `One<>` adapter for trait queries.
* `&dyn Trait` and `&mut dyn Trait` can no longer be used as a `WorldQuery` directly
-- you must explicitly choose between `One<>` and `All<>`.

## Version 0.0.1
