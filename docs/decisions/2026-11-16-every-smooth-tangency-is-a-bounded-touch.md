# Every smooth tangency is a bounded touch

**Decided by the user, 2026-11-16,** choosing "Every tangency is a bounded touch" over "Exact tangency is a Point". The
plan step `intersection.resolution` said a tangent contact is a `Contact::Point` only when the decision certifies it.
Building it, the worker read that as: a Point only where no loop can hide within the tolerance. So every smooth
tangency, such as sphere on sphere, cylinder on plane or Viviani's node, comes back as `BelowResolution { at, bound }`,
a touch with the largest loop it could hide. With that reading P5's near-tangent sweep went from 13 silent failures to
none. The other reading would report a Point wherever exact arithmetic decides the gap is exactly zero, so a tangent
pair on the world's axes would give a Point and the same pair turned slightly would not.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A contact that is no tangency, such as a cone's apex, is a `Contact::Point`. Every smooth tangency is
  `BelowResolution`, with a bound that contains any loop the band could hide there. A tangency along a curve has an
  infinite bound.
- The answer does not depend on where a part is placed.
- `booleans` treats `BelowResolution` as a touch within tolerance of the size its bound gives, never as an exact point.
