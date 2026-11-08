// [class.base.init]/8 (WP-875): a base-class mem-initializer that
// aggregate-initializes the base is checked like a member's: a reference
// member bound to a temporary or an initializer_list member's backing array
// is an error, a pointer member holding a parameter's address warns. A
// delegating mem-initializer is not checked (as in clang).
//@ strict-diags
//@ expect: compile-fail
//@ diag: 29:24 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 28:23 note "reference member declared here"
//@ diag: 31:24 error "backing array for 'std::initializer_list' member 'l' is a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 30:39 note "'std::initializer_list' member declared here"
//@ diag: 33:30 warning "initializing pointer member 'p' with the stack address of parameter 'x'"
//@ diag: 32:17 note "pointer member declared here"
//@ diag: 35:30 error "reference subobject of member 'a' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 34:14 note "member with reference subobject declared here"
//@ diag: 35:35 warning "initializing pointer member 'p' with the stack address of parameter 'y'"
//@ diag: 34:22 note "pointer member declared here"

namespace std {
template <class E> class initializer_list {
  const E *b;
  decltype(sizeof 0) n;
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
};
} // namespace std

struct A { const int &r; };
struct B : A { B() : A{0} {} };
struct L { std::initializer_list<int> l; };
struct M : L { M() : L{{1, 2}} {} };
struct P { int *p; };
struct Q : P { Q(int x) : P{&x} {} };
struct N { A a; int *p; };
struct O : N { O(int y) : N{{1}, &y} {} };
int g;
// A delegating mem-initializer is not checked (clang: no entity lifetime).
struct R { const int &r; R() : R(A{2}.r) {} R(const int &x) : r(x) {} };
struct OK : A, P { OK() : A{g}, P{&g} {} };
