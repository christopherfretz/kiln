// The written-reference table (spec/ast.md §18.4, stage 3), role
// `using-decl`: the name of each using-declarator ([namespace.udecl]/1)
// is one entry after its qualifier's `nns` entries. Its entity is the
// first declaration the using-declarator introduced (`f` names two
// overloads: the first, f(int)); an inheriting constructor's names the
// first inherited constructor ([class.qual]/2, [namespace.udecl]/3); a
// dependent one has none (`dep`). `using enum` names a type (stage 2).
//@ written-refs: using-decls.refs
namespace lib {
int f(int);
int f(long);
struct Widget {};
int count;
enum class Color { red };
namespace detail {
int helper();
}
} // namespace lib

using lib::f;
using lib::Widget, lib::count;
using ::lib::detail::helper;
using enum lib::Color;

struct Base {
  Base(int);
  Base(long);
  int value() const;
  Base &operator=(const Base &);
};
struct Derived : Base {
  using Base::Base;
  using Base::value, Base::operator=;
};

template <class T> struct Wrap : T {
  using typename T::type;
  using T::get;
};
template <class... Ts> struct Overload : Ts... {
  using Ts::operator()...;
};

int use() {
  using lib::detail::helper;
  return f(1) + count + helper();
}
