// WP-1331: before C23, #embed is "a C23 extension" (-Wc23-extensions, on
// by default), not "a Clang extension" (C++'s wording), as in clang 19/22.
//@ std: c17
//@ strict-diags
//@ diag: 9:2 warning "#embed is a C23 extension"
//@ expect: compile-ok

static const unsigned char data[] = {
#embed "Inputs/embed-data.txt"
};
int size = sizeof data;
