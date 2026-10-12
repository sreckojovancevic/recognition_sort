# RangBroj (RankNumber)

## Definition

**RangBroj** is a number whose identity is defined not only by its numeric
value, but also by its **position in an ordered structure**.

Unlike a classical number:

```
3
```

a RangBroj also carries information about the context it sits in:

```
n@3 = 2 < 3 < 4
```

where:

- **3** — the inner identity,
- **2** — the left outer descriptor,
- **4** — the right outer descriptor.

## Core idea

A classical number answers one question:

```
What am I?
```

A RangBroj answers two:

```
What am I?   Where am I?
```

That is why a RangBroj is an **extended identity** of a number.

## Relational identity

In the RangBroj model, identity is not just a value — it is a **relation**.

```
n@3 = 2 < 3 < 4
```

is the unique relational identity of the number 3. A number is no longer an
isolated value, but an element known through its **immediate neighbourhood**.

## Informational self-awareness

A RangBroj is not "aware" in any psychological sense. Here *self-awareness* is
used strictly in the **informational** sense:

```
the object holds information about itself.
```

For a RangBroj that means:

```
I am 3. I sit between 2 and 4.
```

In other words, **position becomes part of identity**.

## The RangBroj thesis

The richer an object's identity, the less searching is needed to determine its
location.

For a RangBroj:

```
Identity + Relation = Position
```

Location is **derived from the description**, not discovered by search.

## Relation to the Recognition approach

The recognition approach organizes elements through:

```
Recognize  →  Determine position  →  Place
```

A RangBroj is a number whose identity **already contains** the information
needed to determine its position. Because of this, order can be built by
**recognition and placement**, rather than by **pairwise comparison** of
elements.

## Summary

A RangBroj is a number **enriched by relation**.

Instead of carrying only a value:

```
3
```

it carries:

```
n@3 = 2 < 3 < 4
```

— a number that, alongside its identity, also holds information about its
**position in a structure**.

In short:

```
RangBroj = Identity + Position
```

## Zero is just a position (a passage, not a wall)

In this model **zero has no privileged status**. Placement is by
`value - origin`, so the sign disappears before placement — zero is simply one
position the sequence passes through, not a boundary. As a RangBroj it is:

```
n@0 = -1 < 0 < 1
```

`src/bin/verbose_demo.rs` shows this directly: `meta(0) = (-1, 0, 1)`, and
negatives sort with no special case (`-5 - (-12) = 7`, like any positive). The
step from negative to positive goes *through* zero smoothly.

Honest scope: what the code confirms is only this one clean thing — that in
ordering/placement zero is a **passage, not a wall**. Nothing more is claimed.

---

*See [`src/bin/rang_broj.rs`](./src/bin/rang_broj.rs) for the dual-identity
(value + rank) demonstration, and [`KONCEPT.md`](./KONCEPT.md) for the governing
concept. Honest boundary: deriving position from value alone works for a dense,
consecutive domain; for arbitrary/sparse sets the relation must be stored as
context (see [`src/bin/pozicioni_broj.rs`](./src/bin/pozicioni_broj.rs)).*
