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
  occur only for the midpoint of two endpoints. Which rounding the
  original used is not established, so this choice is not claimed to be its,
  and the tests pin this one with values on which truncation gives another number.
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
- **Small levels (FMT-070, FMT-082).** A level under four texels on a side is decoded
  from its one block, cropped, with the block's own texel indices; a block that overhangs the edge of a larger level is cropped the same way.
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
  checked against the bytes that remain. A file whose sections after the strata do not run through the high mask,
  which the loader used to accept, is now refused: as truncated where it ends
  early or a count or length outruns the data, and
  as malformed where a count or length is negative. No installed map is among
  them.
- **Digests (FMT-077 to FMT-079).** The three sets the milestone reads (the
  ground textures, the masks, the units' textures) are fixed by a count and
  one combined digest each, which the Rust decoder and the independent script
  reproduce. A digest is a number from which no byte of the game can be
  recovered.

### The textured terrain

The terrain is drawn with the map's own ground textures and masks, blended as
the original's terrain program blends them, and the first slice's height ramp
stays as the fallback. The clauses are REN-092 to REN-117 of
docs/specs/render.md; this section records each choice and its ground.

- **The blend is the original's.** A mask channel's value m weighs
  clamp(2m - 1, 0, 1) (REN-093); the program TTerrain starts from the layer of
  slot 0, blends slots 1 to 4 in turn by the first mask's four channels, and
  lays slot 9 on by its own alpha; TTerrainXP does the same with slots 5 to 8
  by the second mask's channels before slot 9 (REN-094, REN-095). The ground
  is a read of the original's terrain program, effects.scd, member
  effects/terrain.fx, lines 697 to 713 and 728 to 751, stated in this
  project's own words; REN-092 pins the member's size and line count so the
  line numbers stay checkable, and nothing of its text is kept. A slot the map
  leaves unnamed among those its program blends is skipped; REN-096 measures
  that the largest value under such a slot on the installed maps is 34 of 255,
  which weighs nothing, so skipping and weighing are one drawing on every
  installed map.
- **The tiling rule is PROVISIONAL.** One repeat of a layer covers its stored
  scale in world units on each axis (REN-099, REN-100). The original's shader
  multiplies by a factor the engine sets and does not say how the factor comes
  from the stored scale. The rule is the one the FAF map editor's preview
  uses, at commit bac3e5f478e0293d04a4d18cf8dba95b57b5d49d, in
  Assets/Scripts/Ozone SCMAP Code/ScmapEditor.cs, lines 477, 493 to 500, and
  514, and it fits the installed scales: 4 for most layers, and 26.25 and 128
  for slot 9 on SCMP_012 and SCMP_016, on maps 256 units wide. The head
  accepted it as provisional on 2026-11-26. Its evaluator is the first look,
  the head's run of the binary on SCMP_012, SCMP_016, and the replay beside
  the original, which asks whether the size of the ground's pattern matches.
- **The colour rule is this project's choice (REN-101, REN-102).** The
  original names no colour-space conversion. Textures and masks are uploaded
  as stored bytes in a format that applies none, the layers are blended as
  stored numbers, the blend is multiplied by the first slice's light, and the
  product is decoded by the sRGB transfer function, the exact inverse of the
  encoding an sRGB target applies. So on an sRGB target a texel lit at a factor
  of 1 shows its stored value. The first look judges the choice.
- **The mask's orientation is this project's choice (REN-097, REN-098).** A
  mask is looked up across the whole map; its first stored row lies at the
  north edge and its first column at the west edge, with no flip, and the
  lookup clamps at the edges as the original's mask sampler does. The installed
  files do not say which edge a mask starts at, so the first look judges it.
- **The fallback is the whole terrain (REN-111, REN-112, REN-113, REN-114, REN-116).** When
  the program is none of the three, a file the program draws is missing or
  refused, a mask is missing or refused, or the renderer refuses the set, the
  terrain is drawn by the unchanged height ramp with one notice naming the
  first failing item, its file, the reason, and the count. A terrain with one
  layer left out would show a ground the map does not have and hide the cause;
  a run never ends for a refusal. The readings built where the sources were
  silent: a map that leaves slot 0 or slot 9 unnamed falls back (the skip is for
  slots 1 to 8); a named slot whose stored scale is not finite and positive
  falls back, naming the scale; in `view`, an install whose archives do not
  mount makes every file missing, so the ramp is drawn with its notice; and a
  mask or layer the program does not read is neither decoded nor a reason.
- **What the pixel tests prove and what only a look judges (REN-103, REN-108,
  REN-109).** Every pixel test compares read-back pixels with a number worked
  from a clause, within 2 in 255 a channel, with the nearest lookup at texel
  centres, on the hardware adapter and on the software driver, from literal
  pixels. The binary's lookups are filtered, the two paths differing in the
  samplers alone. Filtering, wrap, and the choice of a mip level are judged by
  the first look; the one measurement taken is how far the two adapters'
  frames differ where a level other than the first is chosen, reported and not
  asserted.
- **The glow map's two omissions (REN-110).** SCMP_023 names the glow program,
  drawn here by TTerrain's rule. The original adds two terms, left out as this
  project's omission: layer two's lookup is shifted by an offset (terrain.fx
  lines 780 and 781), and the output's alpha carries a glow term (lines 782 and
  804).
- **Three clauses of the first slice changed, and why.** REN-021's
  statement is scoped to a terrain drawn by the fallback, because the textured
  terrain's pixels are REN-102's. REN-022's one sentence, that the water's
  colour is told from every terrain colour by its blue channel, is narrowed to
  the colours of the height ramp, by the head's ruling of 2026-11-26, since a
  texel may be bluer than the water. REN-035's order gains the map's textures,
  resolved and decoded after the window and before the upload. The sentence of
  the clauses' prose that listed terrain splatting among later slices now says
  what this slice draws. REN-015, REN-024, REN-029, REN-030, REN-031, and the
  rest stand byte for byte: the textured terrain has a shader module and a
  source function of its own (REN-107), and its vertex keeps 24 bytes.
- **The binary's new dependency.** The binary now depends on the decoder crate
  to decode the textures and masks it hands the renderer; the renderer's own
  dependencies stay four, and no loader type crosses into its signatures,
  which take plain pixel buffers, refused before anything is created and
  leaving the renderer as it was (REN-104, REN-105, REN-106).

## Consequences

- A later renderer unit receives decoded levels and masks without trusting a
  header, and can compare its own sampling against the same sets.
- The decoder's memory for a worst-case file is bounded by its data, not by
  its header; a consumer that decodes a whole set at once still pays the sum
  of the decoded sizes, about 3 GiB for the units' two textures,
  unless it decodes one file at a time.
- The rounding is the project's, not the original's; if a side-by-side
  observation shows a last-bit difference that matters, this section is the
  place to revise it, and the three digests change with it.
- The map loader now refuses a file that the earlier one accepted when what
  follows its strata does not run through its masks; the 60 installed maps all do.
- A map's textures cost time at the start of a run: the install's archives
  mount in several seconds on the machine of the first measurement, and the six
  textures and the mask decode in a fraction of that; `replay` mounts the
  archives a second time after the window. Nothing is cached or threaded in
  this version.
