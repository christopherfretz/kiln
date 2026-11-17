// From C++17 (guaranteed copy elision; clang's CWG2327 candidates) the
// conversion function yielding the class initializes the object directly:
// the C++11/14 siblings conversion-ambiguous-cxx11/-cxx14 are ambiguous.
// It stands here because its implicit object conversion (an rvalue bound
// to '&&') is better than that of 'operator const Dest &() const &'; one
// no better than the other conversion function stays ambiguous, see
// conversion-functions-ambiguous-cxx17.cpp.
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
struct Tgt;
struct From { operator Tgt() const; };
struct Tgt { Tgt(); Tgt(const From &); };
void f() {
  Src x;
  Dest d(mv(x));
  From fr;
  Tgt g(fr);
  (void)d; (void)g;
}
