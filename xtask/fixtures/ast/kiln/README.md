`expect: ast` fixtures for the kiln path, run with `--compiler
xtask/fixtures/fake-kiln/kiln` (oracle clang++-19). The fake kiln prints the
canned dump `<test>.ast` for `-fdump-ast`; the harness compares it with the
golden `<test>.expected`, and with `oracle: clang` also compares clang's
normalized dump with the golden. Outcome = file-name prefix, as in
`../../basic/`.
