// [class.prop]/3.6 (CWG1813): a standard-layout class has all its
// non-static data members and bit-fields, unnamed ones included, first
// declared in the same class, so an unnamed bit-field next to a base with
// fields (even a zero-width unnamed one) breaks it. Found running GCC's C++
// testsuite (WP-1467).
struct B { int i; };
struct C : B {};
struct D : C {};
struct E : D { char : 4; };
static_assert(__is_standard_layout(D), "");
static_assert(!__is_standard_layout(E), "");
struct Z { int : 0; };
struct Y : Z { int : 0; };
static_assert(__is_standard_layout(Z), "");
static_assert(!__is_standard_layout(Y), "");
struct F { char : 4; };
struct G : F {};
static_assert(__is_standard_layout(G), "");
