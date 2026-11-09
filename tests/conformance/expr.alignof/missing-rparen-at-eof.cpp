// [expr.alignof]/1, [lex.phases]/1.8: `alignof(` whose `)` is missing at
// the end of the file. The parser reports the missing `)` at EOF; the
// operand's source range then ended at the end-of-file location, which
// kiln measured as a token and stepped past the buffer (an ICE, WP-972).
// The file deliberately ends without a newline: clang puts the EOF token
// on a final newline, kiln after it (a location-only difference).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:20 error "expected ')'"
//@ diag: 12:16 note "to match this '('"
//@ diag: 12:20 error "expected ';' after top level declarator"
int y = alignof(int