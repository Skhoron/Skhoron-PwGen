.# Skhoron-PwGen

A command-line password generator written in Rust. No external dependencies, a single binary.
Random numbers are provided by the custom Skhoron RNG generator based on ChaCha20 (see below).
Passwords are not saved anywhere.

Building

cargo build --release
./target/release/skhoron-pwgen

Interactive mode

Running without arguments prompts for the length, character sets, repetition mode, and quantity:

$ skhoron-pwgen

╔══════════════════════════╗
║      Skhoron-PwGen       ║
╚══════════════════════════╝

Length [24]:

Arguments

skhoron-pwgen --length 32
skhoron-pwgen --length 32 --no-repeat
skhoron-pwgen --length 24 --upper --lower --digits --symbols
skhoron-pwgen --length 20 --count 10
skhoron-pwgen --length 16 --no-similar --exclude "{}[]"
skhoron-pwgen -l 20 -q --clipboard
skhoron-pwgen -l 20 --json --entropy

Option| What it does
"-l, --length N"| length, default 24, maximum 4096
"-c, --count N"| number of passwords, default 1, maximum 10000
"--upper --lower --digits --symbols"| character sets "A-Z", "a-z", "0-9", "!@#$%^&*"; all are used by default if none are specified
"--no-repeat"| characters are not repeated
"--no-similar"| excludes "0 O 1 l I"
"--exclude STR"| excludes all characters from "STR"
"--entropy"| entropy in bits
"--clipboard"| copies to the clipboard using "pbcopy", "clip", "wl-copy", "xclip", or "xsel"
"-q, --quiet"| passwords only, one per line
"--json"| JSON output, including entropy

With "--no-repeat", the length cannot exceed the number of characters in the character set (70 with all sets, 65 with "--no-similar").
In this mode, selection works like drawing without replacement, so entropy is calculated as "log2(n·(n-1)·…)", rather than "length·log2(n)".

Exit codes: 0 for success, 1 for runtime error, 2 for invalid arguments.

Skhoron RNG

The generator in "src/rng.rs" was written from scratch within the project, with no external crates:

- ChaCha20 according to RFC 8439, verified against test vectors from the RFC (Section 2.3.2 and Appendix A.1).
- The state is a 32-byte key. Each ChaCha20 block is split in half: the first 32 bytes become the new key, and the second half is returned; the old key is overwritten. Previously generated passwords cannot be recovered from the state.
- The initial key is obtained from the OS ("/dev/urandom" on Linux and macOS, "BCryptGenRandom" on Windows). Entropy cannot be obtained from an algorithm alone; it requires a physical source, and the OS collects it from interrupts, timings, and the processor's hardware RNG.
- Every 2 MiB of output, a fresh 32 bytes from the OS are mixed into the key.

The cipher itself was not invented: ChaCha20 according to RFC 8439 is used. The RNG construction built on top of it is custom and is not a standard CSPRNG.

Things to consider

- Character selection is unbiased: values from the OS generator are discarded if they fall within the tail of the range that is not divisible by the size of the character set.
- Temporary strings containing generated passwords are zeroed after use. Copies already passed to the terminal or clipboard are not cleared.
- "--clipboard" puts the password in the clipboard. Clipboard history and clipboard managers may save it.
- A password printed to the terminal remains in the terminal scrollback. In scripts, use "-q" and pass the result through a pipeline.
- If the OS cannot provide initial entropy (for example, if "/dev/urandom" is unavailable in a restricted chroot), the program exits with an error rather than substituting something weak.

Development

cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test

"cargo audit" is not needed in CI: there are no dependencies to audit.

License

BSD 3-Clause, see "LICENSE" (LICENSE).

Comment

FE4X - Do not modify the code in the repository