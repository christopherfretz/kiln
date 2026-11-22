// The declaration-extent table (spec/ast.md §19.3): a declaration's full
// range starts at its first token, which may be an attribute, the
// `template` of its head (the requires-clause included), a braceless
// `extern "C"` or a decl-specifier, and ends at its `;` or `}`; each
// redeclaration has its own row.
//@ decl-extents: full-range.extents
//@ std: c++20
[[nodiscard]] int f(int);
int f(int);
inline int f(int x) { return x; }
template <class T>
  requires(sizeof(T) > 1)
T g(T v);
extern "C" void c_fn(void);
extern "C" void c_fn(void) {}
extern "C" {
int c_var;
}
__extension__ long long ext_ll;
struct [[maybe_unused]] Tag;
struct Tag {
  int m;
};
const volatile unsigned cv = 0;
