// Debug info (WP-1424; found compiling the libc++ tests with -g): a
// class-type template argument reached only from a local variable's
// `#dbg_declare` record, of a class that is only constant-evaluated.
// LLVM 19's printer does not walk debug records, so clang's text names
// `%"struct.n::ic"` without defining it and does not verify (listed as
// an oracle-invalid deviation); kiln prints the struct's definition.
//@ args: -g
namespace n {
template <class T, T v> struct ic { constexpr T operator()() const noexcept { return v; } };
using ft = ic<bool, false>;
}
template <auto F> struct W { constexpr bool operator()() const { return !F(); } };
template <auto F> constexpr W<F> nf() { return {}; }
int main() { auto w = nf<n::ft{}>(); return w() ? 0 : 1; }
