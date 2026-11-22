// [temp.deduct.type]/17, [temp.arg.nontype]/1, [dcl.init.list]/7: an
// array bound deduced for a non-type parameter is a std::size_t value
// converted to the parameter's type when the deduction is finished; a
// value the type cannot represent is a narrowing conversion, so the
// candidate is a substitution failure, noted "non-type template argument
// evaluates to 300, which cannot be narrowed to type 'unsigned char'"
// (clang `ConvertDeducedTemplateArgument`). A bound that fits deduces.
// WP-1822. Verified against clang 22 and 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:12 error "no matching function for call to 'f'"
//@ diag: 21:33 note "candidate template ignored: substitution failure: non-type template argument evaluates to 300, which cannot be narrowed to type 'unsigned char'"
//@ diag: 26:12 error "no matching function for call to 'h'"
//@ diag: 24:31 note "candidate template ignored: substitution failure: non-type template argument evaluates to 200, which cannot be narrowed to type 'signed char'"
//@ diag: 29:13 error "no matching function for call to 'f2'"
//@ diag: 27:33 note "candidate template ignored: substitution failure: non-type template argument evaluates to 256, which cannot be narrowed to type 'unsigned char'"
//@ diag: 34:15 error "static assertion failed due to requirement 'n<255U>() == 0'"
//@ diag: 34:25 note "expression evaluates to '255 == 0'"
//@ diag: 35:13 error "no matching function for call to 'f'"
//@ diag: 21:33 note "candidate template ignored: substitution failure: non-type template argument evaluates to 256, which cannot be narrowed to type 'unsigned char'"
template <unsigned char N> void f(int (&)[N]);
int a[300];
void g() { f(a); }
template <signed char N> void h(int (&)[N]);
int b[200];
void k() { h(b); }
template <unsigned char N> void f2(int (*)[N]);
int c[256];
void g2() { f2(&c); }
int d[255];
template <unsigned char N> constexpr unsigned n(int (&)[N]) { return N; }
template <unsigned M> constexpr unsigned n() { return M; }
static_assert(n(d) == 255);
static_assert(n<255U>() == 0);
void g3() { f(c); }
