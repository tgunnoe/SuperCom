# ADR-0018: Textures on the map

**Status:** PROPOSED

**Date:** 2026-11-26

## Context

The renderer draws terrain and units as flat colour, and the installed game
supplies textures for both: ground textures named by each map's strata, two
blend masks embedded in each map, and two textures for each unit. They are
block-compressed `.dds` files (DXT1, DXT3, DXT5) and a few uncompressed
32-bit ones. Before anything samples them, the project needs their pixels,
and the map's masks, which the map loader stops short of today.

The texture loader reads headers only: it returns the width, height, and mip
count as stored and checks none of them against the data (FMT-044), which is
the texture half of issue 18. A consumer that sizes a buffer from those words
would allocate whatever a hostile file asks for. This record holds the
choices of the two loaders, the texture loader and the map loader, and nothing
about drawing; later work adds its own sections under Decision.

## Decision

### The loaders

- **The decoder is written here and decodes on the CPU.** The crate that reads
  the header also decodes the pixels of every level into 8-bit red, green,
  blue, alpha, four bytes a pixel, rows in stored order without a flip
  (FMT-064). The block layouts are a published format (FMT-061, FMT-062);
  writing the decoder here adds no dependency, keeps the loader headless
  (AGENTS.md I4), and lets a second decoder in another language check it
  (FMT-077 to FMT-079). The GPU can sample the compressed forms directly
  later; that is a renderer decision and is not made here.
- **Integer arithmetic only (FMT-073).** Floating point is absent from the
  decoder, so every architecture computes the same bytes by construction,
  which the determinism requirement (R2) asks of anything a replay could
  depend on.
- **The rounding rule and its source (FMT-065).** The Khronos Data Format
  Specification (version 1.3, chapter 18) and the Microsoft block-compression
  page state the interpolated colours and alphas as exact quotients
  and say nothing of how a quotient becomes an 8-bit integer; the
  OpenGL extension for S3TC says the same. The sources do not contradict one
  another; they are silent. The rule chosen here is: expand each endpoint to
  8 bits by replicating its high bits (a 5-bit `v` becomes `(v << 3) | (v >>
2)`, a 6-bit one `(v << 2) | (v >> 4)`), then take each quotient of 8-bit
  values to the nearest integer, a tie going up, as `(n + d / 2) / d`. Ties
  occur only for the midpoint of two endpoints. Hardware decoders differ in
  the last bit of an interpolated channel, so no choice is the original's, and
  the tests pin this one with values on which truncation gives another number.
- **The colour block of DXT3 and DXT5 (FMT-067, FMT-068).** It is decoded in
  the four-colour form whatever the order of the endpoints, which is the
  wording of the OpenGL extension (FMT-063); its implementation note on
  hardware that does not ignore the order is the known divergence. The census
  of the three sets found no installed block where the choice matters
  (FMT-080).
- **DXT1 (FMT-066).** The endpoints are compared as 16-bit unsigned integers;
  equal endpoints take the three-colour form. Code 3 of that form is black
  with alpha 0, as the Khronos page gives for BC1 with alpha; every other
  DXT1 pixel has alpha 255.
- **Uncompressed 32-bit through the masks (FMT-069).** The masks of the pixel
  format place each channel; each must be one byte-aligned run of eight bits
  and no two may coincide, and a zero alpha mask reads as opaque. This covers
  every installed uncompressed texture the milestone reads and refuses the
  rest by name rather than guessing.
- **Small levels (FMT-070).** A level under four texels on a side is decoded
  from its one block, cropped, with the block's own texel indices.
- **What is refused, before anything is sized (FMT-071, FMT-072).** The
  decoder refuses a zero side or one over 8,192 (the largest 2D texture side
  of wgpu's default limits, and four times the installed maximum of 2,048), a
  mip count of zero or beyond one more than the base-2 logarithm of the longer
  side (the installed maximum is 12 levels, on a 2,048 side), a cube map or a
  volume, a FOURCC other than the three, an uncompressed texture that is not
  32 bits or whose masks are not as above, and data that does not hold the
  whole chain. Bytes after the chain are allowed (FMT-064). The allocation
  size comes from one pure function of the header and the data length, and
  cannot exceed 8 bytes for each byte of data, so a file cannot ask for more
  memory than it carries data for. `Header::parse` is unchanged.
- **What the map loader returns and leaves unread (FMT-074 to FMT-076).** The
  map loader reads on past the strata through the nine normal slots, the
  decals, the decal groups, and the embedded normal maps, and returns the two
  masks' bytes and the terrain program's name. It does not look inside a mask
  (the caller decodes it) and it returns no normal slot, decal, decal group,
  or embedded normal map; each is read past with its counts and lengths
  checked against the bytes that remain. A file that ends before the masks,
  which the loader used to accept, is now refused as truncated: that is the
  one change to what an existing input returns.
- **Digests (FMT-077 to FMT-079).** The three sets the milestone reads (the
  ground textures, the masks, the units' textures) are fixed by a count and
  one combined digest each, which the Rust decoder and the independent script
  reproduce. A digest is a number from which no byte of the game can be
  recovered.

## Consequences

- A later renderer unit receives decoded levels and masks without trusting a
  header, and can compare its own sampling against the same sets.
- The decoder's memory for a worst-case file is bounded by its data, not by
  its header; a consumer that decodes a whole set at once still pays the sum
  of the decoded sizes, about 3 GiB for the units' two textures, and decodes a
  file at a time.
- The rounding is the project's, not the original's; if a side-by-side
  observation shows a last-bit difference that matters, this section is the
  place to revise it, and the three digests change with it.
- The map loader now fails a file that the earlier one accepted if it ends
  before its masks; the 60 installed maps all carry them.
