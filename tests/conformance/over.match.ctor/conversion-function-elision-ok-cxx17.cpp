// [over.match.ctor]/1, [dcl.init.general]/16.6.2: the constructors alone
// select the move constructor reached through the one conversion function
// yielding the class, and from C++17 that conversion function initializes
// the object directly, also when the move constructor is deleted (GCC 16
// and clang 22 agree). Sibling of conversion-functions-ambiguous-cxx17.cpp.
// A conversion function template reaching both the move constructor and
// another constructor ('Path(Str&&)') leaves the constructors alone
// ambiguous by the text, but GCC 16 and clang 22 let the conversion
// function yielding 'Path' win, and real code relies on it (reduced from
// a JSON library's tests).
//@ std: c++17
//@ expect: compile-ok
struct NoMove {
  NoMove(int);
  NoMove(NoMove &&) = delete;
};
struct ToNoMove { operator NoMove(); };
NoMove a(ToNoMove{});
NoMove b = NoMove(ToNoMove{});
struct V { int n; };
struct OneConv { operator V() const; };
void f(const OneConv &o) {
  V v(o);
  (void)v;
}
struct Str { Str(); };
struct Path {
  Path(const Path &);
  Path(Path &&) noexcept;
  Path(Str &&, int = 0);
};
struct Any {
  template <class V> operator V() const;
};
void g() {
  Path p(Any{});
  Path q = Path(Any{});
  (void)p; (void)q;
}
