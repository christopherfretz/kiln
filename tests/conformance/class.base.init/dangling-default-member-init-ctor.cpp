// [class.base.init]/11 (WP-874): a default member initializer that binds a
// reference member (or a std::initializer_list member's backing array) to a
// temporary is ill-formed when a constructor uses it, whether the
// constructor is user-provided or implicit, or the member is the subobject
// of an aggregate member mem-initializer; the error is at the constructor
// (the mem-initializer for subobjects) with a note at the initializer.
//@ strict-diags
//@ expect: compile-fail
//@ diag: 34:30 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 34:23 note "initializing field 'r' with default member initializer"
//@ diag: 34:37 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 34:23 note "initializing field 'r' with default member initializer"
//@ diag: 34:59 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 34:23 note "reference member declared here"
//@ diag: 36:30 error "reference subobject of member 'a' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 32:23 note "initializing field 'r' with default member initializer"
//@ diag: 36:35 error "backing array for 'std::initializer_list' subobject of member 'c' is a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 33:39 note "initializing field 'l' with default member initializer"
//@ diag: 35:8 error "reference member 'r' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object"
//@ diag: 38:3 note "in implicit default constructor for 'T' first required here"
//@ diag: 35:23 note "initializing field 'r' with default member initializer"

namespace std {
template <class E> class initializer_list {
  const E *b;
  decltype(sizeof 0) n;
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
};
} // namespace std

struct A { const int &r = 1; };
struct C { std::initializer_list<int> l = {1, 2}; };
struct S { const int &r = 2; S() {} S(int) {} S(long) : r(3) {} };
struct T { const int &r = 4; };
struct B { A a; C c; B() : a{}, c{} {} };
struct U { int k = 5; const int &r = k; U() {} }; // binds a member: fine
T t;
