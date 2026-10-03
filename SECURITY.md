# Security Policy

## Supported versions

This project is pre-1.0 homebrew. Only the current default branch is supported.

## Reporting a vulnerability

Please report security issues privately rather than in a public issue. Use the
security advisory form on the repository, or contact the maintainer.

## Scope

This is a single-player game that runs on a console or in an emulator. The
attacker model is narrow, but the following are in scope:

- **Save-file handling.** Malformed or corrupt memory-card data must be rejected
  without panicking. `plattypus-core::save` treats all untrusted input as
  hostile: a payload shorter than `SERIALIZED_SIZE` must not parse, and a bad
  checksum must classify as corrupt rather than loading.
- **ISO 9660 / MDEC parsing.** Disc sector data is length-checked before any
  field is read. A malformed directory record must not cause an out-of-bounds
  read or a decode overrun.
- **Denial of service in the player.** Any unbounded wait must be bounded so a
  failed disc read cannot hang the console indefinitely.
- **Web Arcade bundle.** The browser build fetches a disc image; it must not
  execute anything from the served ROM.

Out of scope: vulnerabilities in the vendored PSoXide SDK (report upstream at
https://github.com/EBonura/PSoXide) and in third-party assets.
