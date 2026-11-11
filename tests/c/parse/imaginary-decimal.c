// C17 6.2.5p11, G.2: _Imaginary is reserved for the optional imaginary
// types, which clang does not support (the type stays real); the
// _Decimal32/64/128 keywords (C23 6.2.5p12) are rejected in every C mode.
// The default -std=c17 is pinned here; c99, c23 and gnu17 give the same.
//@ args: -pedantic
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:8 error "imaginary types are not supported"
//@ diag: 21:1 warning "plain '_Complex' requires a type specifier; assuming '_Complex double'"
//@ diag: 21:1 error "imaginary types are not supported"
//@ diag: 22:1 warning "complex integer types are a GNU extension"
//@ diag: 22:5 error "imaginary types are not supported"
//@ diag: 23:19 error "cannot combine with previous 'imaginary' declaration specifier"
//@ diag: 23:8 error "imaginary types are not supported"
//@ diag: 24:1 error "GNU decimal type extension not supported"
//@ diag: 25:1 error "GNU decimal type extension not supported"
//@ diag: 26:1 error "GNU decimal type extension not supported"
//@ diag: 27:1 error "'_Complex _Decimal64' is invalid"
//@ diag: 27:10 error "GNU decimal type extension not supported"
double _Imaginary di;
_Imaginary x;
int _Imaginary ii;
double _Imaginary _Complex b;
_Decimal32 d32;
_Decimal64 d64;
_Decimal128 d128;
_Complex _Decimal64 cd;
double _Complex dc;
_Static_assert(sizeof di == sizeof(double), "");
