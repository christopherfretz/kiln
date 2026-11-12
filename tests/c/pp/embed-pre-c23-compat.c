// WP-1331: at C23 #embed is a -Wpre-c23-compat warning (off by default),
// as in clang 19/22.
//@ std: c23
//@ args: -Wpre-c23-compat
//@ strict-diags
//@ diag: 10:2 warning "#embed is incompatible with C standards before C23"
//@ expect: compile-ok

static const unsigned char data[] = {
#embed "Inputs/embed-data.txt"
};
int size = sizeof data;
