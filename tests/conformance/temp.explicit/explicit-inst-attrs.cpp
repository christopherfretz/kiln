// [temp.explicit], [dcl.attr.grammar]/5: a standard attribute before an
// explicit instantiation's decl-specifiers (also after GNU ones) is "an
// attribute list cannot appear here" once, at its first `[[`; one after
// the class-key of a class's explicit instantiation is the same error at
// each attribute's name (clang `ProhibitCXX11Attributes`). A GNU
// `deprecated` on an explicit instantiation is the specialization's: a
// use warns, with the note at the attribute. kiln used to accept the
// standard ones and drop the GNU one (reduced from real code; WP-2541).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:10 error "an attribute list cannot appear here"
//@ diag: 25:17 error "an attribute list cannot appear here"
//@ diag: 26:19 error "an attribute list cannot appear here"
//@ diag: 27:19 error "an attribute list cannot appear here"
//@ diag: 27:31 error "an attribute list cannot appear here"
//@ diag: 27:48 error "an attribute list cannot appear here"
//@ diag: 28:10 error "an attribute list cannot appear here"
//@ diag: 30:32 error "an attribute list cannot appear here"
//@ diag: 32:14 warning "'f<unsigned int>' is deprecated"
//@ diag: 31:25 note "'f<unsigned int>' has been explicitly marked deprecated here"
template <class T> void f(T) {}
template <class T> struct C {};
template <class T> T v = T();
template [[gnu::weak]] void f<char>(char);
extern template [[gnu::weak]] void f<int>(int);
template struct [[deprecated]] C<char>;
template struct [[deprecated, maybe_unused]] [[gnu::packed]] C<int>;
template [[deprecated]] int v<int>;
template void f<short> [[gnu::weak]] (short);
template __attribute__((weak)) [[gnu::weak]] void f<long>(long);
template __attribute__((deprecated)) void f<unsigned>(unsigned);
void use() { f(1u); }
