// [expr.new]/2: a placeholder in a new-type-id's type-specifier-seq may sit
// under a ptr-operator ('new auto*(e)'), deduced from the parenthesized
// initializer as for a variable 'auto *x = e'.
//@ xfail(kiln): WP-1134 follow-up, a placeholder under a new-declarator ('new auto*(e)') is unsupported
//@ std: c++17
int **g() { return new auto*(new int); }
