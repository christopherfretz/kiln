// [class.bit]/1 (CWG2229): an unnamed bit-field is not cv-qualified,
// directly or through a typedef; reported at the colon (WP-1015).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:23 error "anonymous bit-field cannot have qualifiers"
//@ diag: 11:41 error "anonymous bit-field cannot have qualifiers"
//@ diag: 12:16 error "anonymous bit-field cannot have qualifiers"
//@ diag: 13:26 error "anonymous bit-field cannot have qualifiers"
typedef const int CI;
struct S1 { const int : 3; volatile int : 2; };
struct S2 { CI : 2; };
struct S3 { volatile int : 0; };
