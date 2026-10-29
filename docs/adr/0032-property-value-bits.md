# ADR-0032: Property value bits

- **Status:** Proposed
- **Date:** 2026-10-29
- **Crates:** `ltk_wwise`
- **Related:** #310, LeagueToolkit/ltk-manager#949, LeagueToolkit/ltk-manager#957,
  `docs/design/wwise-audio.md` [section 9.2](../design/wwise-audio.md#s9.2) and
  [section 9.7](../design/wwise-audio.md#s9.7)

## Context and problem statement

A `PropBundle` holds a `u8` property ID and a 4-byte value per property. The Wwise reader reads the
value as `AkPropValue`, a union of `f32`, `i32` and `u32`. The property ID selects the member:
Volume and Pitch are `f32`, DelayTime and Loop are integers, AttenuationID is an object ID. The
wire carries no type tag. A RandomSequenceContainer carries three more `f32` fields, its
transition time and the bounds of its random offset.

The typed objects of `ltk_wwise` derive `Eq`, and `Typed` derives `Eq`. A typed object decodes
and encodes back to its payload byte for byte, over 768,553 shipped objects. A value of an ID
that the crate does not name has no known type. A shipped `f32` value can be a NaN, and an `f32`
NaN is not equal to itself.

## Decision drivers

- Every shipped object encodes back to its payload, including values of unnamed IDs.
- `Typed`, `Sound`, `Action` and `Container` keep `Eq`.
- A consumer reads a Volume as an `f32` and a Loop as an `i32` without reinterpreting bits by
  hand.

## Considered options

1. **A `u32` newtype of the bits**, with `f32`, `i32` and `u32` accessors that the type of the
   ID selects.
2. **An enum of the member**, `F32(f32)`, `I32(i32)` or `U32(u32)`, chosen at decode time by the
   type of the ID.
3. **`f32` fields**, with every value stored as an `f32`.

## Decision

**Option 1. A property value is `PropValue(pub u32)`, the bits as read. `PropertyId::value_type`
gives the type of a named ID. `PropBundle::f32`, `PropBundle::i32` and `PropBundle::u32` read the
value of an ID as that type and give `None` for an ID of another type. The three transition times of a
RandomSequenceContainer are `PropValue`s.**

`docs/design/wwise-audio.md` [section 9.2](../design/wwise-audio.md#s9.2) specifies the bundles
and the ID table.

## Consequences

- **Positive:** every value round-trips, NaN payloads included. `Eq` and `Hash` hold on every
  typed object. A value of an unnamed ID needs no type.
- **Negative:** a field that the client reads as an `f32` holds a `u32` in Rust. A caller that
  reads `.0` of a Volume gets bits, not decibels. The type of an ID comes from the crate table. A
  misnamed ID in the table reads with the wrong type.
- **Revisit when:** a Wwise version adds a property bundle with a type tag on the wire.

## Pros and cons of the options

### Option 2: an enum of the member

- Good: a matched value carries its type. A consumer cannot read an `f32` as bits by accident.
- Bad: a value of an unnamed ID has no member, and needs a third variant of raw bits. `F32(f32)`
  loses `Eq` on every type that holds a bundle. An edit that sets the wrong member for an ID
  encodes without an error and decodes back as the other member.

### Option 3: `f32` fields

- Good: the most common values, volume and pitch, read with no accessor.
- Bad: an integer value read as an `f32` is a denormal and prints as noise. `Typed` loses `Eq`,
  a change to the public contract of 0.1.0 with no gain for the integer properties.
