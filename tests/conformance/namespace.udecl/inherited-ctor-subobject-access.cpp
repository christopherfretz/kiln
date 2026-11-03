// [namespace.udecl]/19: the constructors a using-declarator inherits are
// accessible if they would be accessible when used to construct an object
// of the base class. That holds for a member or base subobject initialized
// through an inheriting constructor too: the access checked is the base
// constructor's, named in the base class, from the context of the
// mem-initializer, so a friend of the base may use a private one (libstdc++
// 16 <format>: `_Scanner<_CharT>::_Parse_context` inherits the private
// `basic_format_parse_context(string_view, size_t)` its befriended
// enclosing class initializes `_M_pc` with; WP-525).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:50 error "field of type 'struct H' has private constructor"
//@ diag: 32:11 note "implicitly declared private here"
//@ diag: 38:23 error "base class 'HB' has private constructor"
//@ diag: 32:11 note "implicitly declared private here"
//@ diag: 43:15 error "calling a private constructor of class 'F'"
//@ diag: 32:11 note "implicitly declared private here"
template <class T> struct Sc;
template <class T> class B {
public:
  constexpr explicit B(int) {}
private:
  friend Sc<T>;
  constexpr explicit B(int, long) {}
};
template <class T> struct Sc {
  struct PC : B<T> { using B<T>::B; int t = 0; } pc;
  constexpr Sc(int a, long n) : pc(a, n) {}
};
Sc<char> s(1, 2);

class F { F(int, long); friend struct G; friend struct M; public: F(int); };
struct G { struct H : F { using F::F; } h; G() : h(1, 2) {} };
struct K { struct H : F { using F::F; } h; K() : h(1, 2) {} };

struct HB : F { using F::F; };
struct M : HB { M() : HB(1, 2) {} };
struct N : HB { N() : HB(1, 2) {} };

class P { protected: P(int, long); public: P(int); };
struct HP : P { using P::P; };
struct Q : HP { Q() : HP(1, 2) {} };
void v() { HB a(1, 2); }
