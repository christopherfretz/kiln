// From C++17 (guaranteed copy elision; clang's CWG2327 candidates) the
// conversion function yielding the class initializes the object directly:
// the C++11/14 siblings conversion-ambiguous-cxx11/-cxx14 are ambiguous.
//@ std: c++17
//@ expect: compile-ok
template <class T> T &&mv(T &t) { return static_cast<T &&>(t); }
struct Dest {
  Dest() = default;
  Dest(Dest &&) = default;
  Dest(const Dest &) = delete;
};
struct Src {
  Dest val;
  operator Dest() && { return mv(val); }
  operator const Dest &() const & { return val; }
};
struct Val { int n; };
struct Two {
  operator Val() const { return {1}; }
  operator const Val &() const { static Val v{2}; return v; }
};
struct Tgt;
struct From { operator Tgt() const; };
struct Tgt { Tgt(); Tgt(const From &); };
void f() {
  Src x;
  Dest d(mv(x));
  Two t;
  Val v(t);
  From fr;
  Tgt g(fr);
  (void)d; (void)v; (void)g;
}
