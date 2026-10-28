// [cpp.error]: #warning emits a diagnostic and preprocessing continues.
// (A header is a test only because it carries a directive header.)
//@ expect: pp
//@ strict-diags
//@ diag: 6:2 warning "this header is a test"
#warning this header is a test
after_warning
