// [over.match.ctor] with C++14 [dcl.init]/17.6.2: before C++17 a
// direct-initialization from another class calls a constructor whose
// parameter is initialized by a user-defined conversion; a copy constructor
// reached through 'operator const T&' and a move constructor through
// 'operator T' are indistinguishable ([over.ics.rank]/3.3: different
// conversion functions), so the call is ambiguous. A converting
// constructor taking the source exactly still wins. Found running GCC's
// C++ testsuite.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:8 error "call to constructor of 'Dest' is ambiguous"
//@ diag: 21:3 note "candidate constructor"
//@ diag: 22:3 note "candidate constructor has been explicitly deleted"
//@ diag: 41:7 error "call to constructor of 'Val' is ambiguous"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor)"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor)"
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
