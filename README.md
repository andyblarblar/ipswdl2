# ipswdl2

ipswdl2 is a CLI for downloading Apple IPhone SoftWare (IPSW) files
using the [ipsw.me](https://ipsw.me) API.

## Usage

To simply download the latest version of all firmware files, use:
`ipswdl2 -A`. Alternatively, you can use `-f <term>` to only filter devices,
or `-L` to list all devices.

If you wish to enable logging, add the `-l <path>` option.

More options can be seen with `-h`.

## Examples

### Download all and log
`ipswdl2 -l '.\logs.txt' -A`

### Download all iPhones
`ipswdl2 -f 'iPhone'`

### Download M1 iMac firmware, deleting old firmware in the process
`ipswdl2 -f 'iMac' -d`

## Installation
Currently, `cargo install ipswdl2` is the easiest way to install. 
Alternatively, one can build this repository using `cargo build --release` at
the root.

*GitHub releases are TBD*