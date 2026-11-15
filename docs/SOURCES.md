# Sources

Where the kernel's algorithms come from. Every file in a kernel crate starts with `//! Source: <key>`, using a key from
this table, or `//! Source: original` when nothing was taken from elsewhere. `gates provenance` holds the two together.

Algorithms come from published papers and books, under
[the provenance rules](decisions/2026-11-08-licence-and-provenance.md). A row of kind `code` is a port from another
project: its file must also be declared in `REUSE.toml`, and its licence must be on the allow-list in `deny.toml`
([no copyleft](decisions/2026-11-08-no-copyleft.md)).

| Key | Kind | Licence | Citation |
|---|---|---|---|
| `fnv-1a` | spec | n/a | Fowler, Noll, Vo, Eastlake, Hansen. *The FNV Non-Cryptographic Hash Algorithm*. RFC 9923, IETF |
| `crc-32` | spec | n/a | The CRC-32 of ISO/IEC 3309 and ITU-T V.42, as W3C, *Portable Network Graphics (PNG) Specification*, annex D, gives it; eight bytes at a time as Kounavis, Berry, *A Systematic Approach to Building High Performance Software-Based CRC Generators*, ISCC 2005, describe |
| `splitmix64` | paper | n/a | Steele, Lea, Flood. *Fast Splittable Pseudorandom Number Generators*. OOPSLA 2014 |
| `shewchuk-1997` | paper | n/a | Shewchuk. *Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates*. Discrete & Computational Geometry 18(3), 1997 |
| `rump-1999` | paper | n/a | Rump. *Fast and Parallel Interval Arithmetic*. BIT Numerical Mathematics 39(3), 1999 |
| `cody-waite-1980` | book | n/a | Cody, Waite. *Software Manual for the Elementary Functions*. Prentice-Hall, 1980 |
| `arm-optimized-routines` | code | MIT OR Apache-2.0 WITH LLVM-exception | Arm Limited. *optimized-routines* v26.07, `math/exp.c` and `math/exp_data.c`, <https://github.com/ARM-software/optimized-routines>, as phasekit ported it ([the decision](decisions/2026-11-11-exp-from-arm.md)) |
| `payne-hanek-1983` | paper | n/a | Payne, Hanek. *Radian Reduction for Trigonometric Functions*. ACM SIGNUM Newsletter 18(1), 1983 |
| `ericson-2005` | book | n/a | Ericson. *Real-Time Collision Detection*. Morgan Kaufmann, 2005 |
| `piegl-tiller-1997` | book | n/a | Piegl, Tiller. *The NURBS Book*, 2nd edition. Springer, 1997 |
| `do-carmo-1976` | book | n/a | do Carmo. *Differential Geometry of Curves and Surfaces*. Prentice-Hall, 1976 |
| `davis-rabinowitz-1984` | book | n/a | Davis, Rabinowitz. *Methods of Numerical Integration*, 2nd edition. Academic Press, 1984 |
| `mantyla-1988` | book | n/a | Mäntylä. *An Introduction to Solid Modeling*. Computer Science Press, 1988 |
| `hormann-agathos-2001` | paper | n/a | Hormann, Agathos. *The Point in Polygon Problem for Arbitrary Polygons*. Computational Geometry 20(3), 2001 |
| `patrikalakis-maekawa-2002` | book | n/a | Patrikalakis, Maekawa. *Shape Interrogation for Computer Aided Design and Manufacturing*, chapter 5, *Intersection Problems*. Springer, 2002 |
| `sarraga-1983` | paper | n/a | Sarraga. *Algebraic Methods for Intersections of Quadric Surfaces in GMSOLID*. Computer Vision, Graphics, and Image Processing 22(2), 1983 |
| `bajaj-hoffmann-lynch-hopcroft-1988` | paper | n/a | Bajaj, Hoffmann, Lynch, Hopcroft. *Tracing Surface Intersections*. Computer Aided Geometric Design 5(4), 1988 |
| `barnhill-kersey-1990` | paper | n/a | Barnhill, Kersey. *A Marching Method for Parametric Surface/Surface Intersection*. Computer Aided Geometric Design 7(1–4), 1990 |
| `von-neumann-1950` | book | n/a | von Neumann. *Functional Operators, Volume II: The Geometry of Orthogonal Spaces*. Annals of Mathematics Studies 22, Princeton University Press, 1950 |
