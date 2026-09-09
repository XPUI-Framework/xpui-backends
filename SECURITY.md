# Security

## Reporting a vulnerability

Report it privately, through **Report a vulnerability** under this
repository's Security tab. That opens an advisory only the maintainers can
read. Do not open a public issue for anything exploitable.

Say what you found, where, and how to reproduce it. A proof of concept helps;
a fix is welcome and not expected.

## What to expect

- An acknowledgement within seven days.
- A fix, or a reasoned decision that none is needed, within ninety days —
  sooner where the risk warrants it.
- Credit in the advisory, unless you would rather not be named.

## Supported versions

`main`. Once a version is published to crates.io, the latest published version
too; older ones are not.

## What counts

This is UI code for e-ink firmware. The likely finding is memory unsafety in an
`unsafe` block or across the C ABI, or a panic reachable from input a device
receives. Both are in scope. A denial of service that needs physical access to
the board is not a vulnerability in this code.
