// The written-reference table (spec/ast.md §18.4, §18.5): type, template,
// base and mem-init names under the stage-1 rules. A name from a macro
// body or argument records the expansion location (`exp=`); a name that
// disambiguation looked at ([stmt.ambig], a parenthesized type or
// expression, `sizeof`) is recorded once, by the parse that is kept; a
// default argument, a default template argument and a default member
// initializer are recorded once, at their written site, however often
// they are used; a template instantiated twice adds no entries; lambda
// and requires-expression parameters are recorded like any other.
//@ written-refs: type-combined.refs
namespace lib {
struct Rec {
  int n;
  Rec(int v = 0) : n(v) {}
};
template <class T = Rec> struct Box {
  T v = T();
};
} // namespace lib

#define REC lib::Rec
#define PTR(T) T*

using lib::Rec;

REC made;
PTR(Rec) pointer = &made;

int defaults(int k = sizeof(lib::Rec)) { return k; }

struct Holder {
  Rec r = Rec(2);
  lib::Box<> b;
};

int tentative(int x) {
  Rec(y);                            // a declaration of `y`
  int a = Rec(x).n;                  // an expression
  int c = sizeof(Rec) + sizeof(x);   // a type and an expression
  int d = ((Rec)x).n + (x);          // a cast and a parenthesized name
  return a + c + d + y.n + defaults() + defaults(1);
}

template <class T> T twice(T t) { return T(t.n * 2); }
int instantiated = twice(Rec(1)).n + twice(lib::Rec(2)).n;

auto lambda = [](Rec r, lib::Box<Rec> const& b) -> Rec { return Rec(r.n + b.v.n); };

template <class T>
concept HasN = requires(T t, lib::Box<T> b) { t.n + b.v.n; };
static_assert(HasN<Rec>);
