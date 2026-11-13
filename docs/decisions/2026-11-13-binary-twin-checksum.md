# The binary twin ends with a checksum, and its reader is held against hostile files

**Decided by the user, 2026-11-13,** asked during [`construction`](../plan/construction.md) once
[the binary twin](2026-11-12-binary-twin.md) had landed. The user asked: "what are the implications of the file header
\- someone could spoof a file, would that really be an issue?" The answer given:

- **The header is a label, not a lock.** Its ten bytes tell a twin from a JSON file, show a file mangled as text in
  transit, and carry the version. Anyone can write them, as anyone can write the start of a PNG or a zip file, and
  writing them gets a file no further than the reader. What matters is what the reader does with a file made to break
  it. It already trusted nothing: every model it reads is checked as `commit` checks an edit, a file cut short, run on,
  or of another version is refused, records do not nest, and serde caps what a claimed length makes it reserve.
- **Three risks were left.** A valid but huge file costs time and memory in proportion to its size, as any format's
  does. There was no checksum, so one flipped bit in a coordinate gives a different model, which may pass every check;
  the JSON has the same gap. And who wrote a file is out of any format's reach: that is signing, a feature of its own.
- **The recommendation.** Keep the header. Add tests of hostile files, with no new crate. And, the user's call because
  it changes the format and is cheap only before release: a CRC-32 at the end of the binary twin, a few dozen lines,
  with no new crate.

The user said: "yes add the checksum and the hostile-file tests, keep the header".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **The twin's last four bytes** are the CRC-32 of every byte before them, the magic, the version and the records,
  little-endian. It is the CRC-32 that PNG, zip and gzip use (ISO/IEC 3309; [`crc-32`](../SOURCES.md)), so any tool can
  check a file: the bytes `123456789` give `0xCBF43926`. It is written by hand, eight bytes at a time through
  tables built when the crate is compiled.
- **The checksum is read before the records.** A reader takes the magic, then the version, since another version may
  end otherwise, so `io.other-version` comes first; then the checksum; and only then the records. A mismatch is the new
  `io.damaged`. A CRC-32 catches every error of one bit, every burst of 32 bits or fewer, and all but one in 2³² of
  the rest. A twin cut short or run on now fails its checksum; `io.not-readable` names the byte where reading stopped
  when the checksum is good and the records are not.
- **Within version 1.** No twin had been released or recorded, so the layout changed without a new version.
- **The JSON gets no checksum.** It stays a file people may read and edit by hand.
- **Hostile files are tests,** in `crates/occam-io/src/tests/hostile.rs`: a twin cut short at every length or run on;
  every one of its bits flipped; and, beneath a checksum made good again, every table and the format's name claiming
  2⁶⁰ elements or as many as a length can say, links to nothing, to the wrong kind or of generation 0, numbers no
  record or family takes, records joined into loops and shells that do not close, thousands of bytes changed at random,
  and records of pure noise. Each is refused with a code, or loads as a model that passed every check, and none panics
  or hangs. An edge's deviation as written is measured again on load, as `commit` measures it, so a file's deviation,
  NaN included, is never trusted.
- **What stays out of reach.** A file's size: no length a file claims is believed, but a large valid model still costs
  the time and memory it holds. A file's author: that is signing, not asked for.
