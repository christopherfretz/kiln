// [basic.lookup.qual.general]/4.2, /4.6, [expr.ref]/4: in
// `p->N::C<int>::~CB<int>()` the template-name `CB` is looked up in the
// prefix `N` of the qualifier (4.2), where the alias template `N::CB`
// names `N::C<int>`, the qualifier's class (4.6): the call destroys `*p`.
// kiln follows the standard and GCC 16; clang 19/22 look the name up only
// in the qualifier's class `N::C<int>` ("no template named 'CB' in
// 'N::C<int>'; did you mean 'N::CB'?"), though they look a non-template
// name up in the prefix (`p->N::C::~D()` with `N::D` a typedef; WP-1152).
// A deliberate divergence (WP-1173; spec/sema-class.md).
//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 19/22 do not look a destructor template-name up in the qualifier's prefix (WP-1173)
//@ stdout: ~C
//@ stdout: ~C
extern "C" int puts(const char*);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
namespace N {
template <class X> struct C { ~C() { puts("~C"); } };
template <class X> using CB = C<X>;
}
int main() {
  alignas(N::C<int>) unsigned char buf[sizeof(N::C<int>)];
  N::C<int>* p = ::new (buf) N::C<int>;
  p->N::C<int>::~CB<int>();
  p = ::new (buf) N::C<int>;
  p->::N::C<int>::~CB<int>();
}
