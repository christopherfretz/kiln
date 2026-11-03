// clang's `[[clang::trivial_abi]]` (WP-460; clang
// `checkIllFormedTrivialABIStruct`, `canPassInRegisters`): it applies to
// classes and unions and takes no arguments; it is dropped, with a warning
// and a note naming the first reason, from a polymorphic class, a class
// with a base that cannot be passed in registers or a virtual base, a
// field that cannot be, or (unless trivially relocatable) with every copy
// and move constructor deleted; no warning for a template instantiation.
// Otherwise the class is trivial for the purposes of calls
// ([class.temporary]/3), and so is a class it is a subobject of.
// [dcl.attr.grammar]/5 (implementation-defined attributes).
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:10 warning "'trivial_abi' cannot be applied to 'D'"
//@ diag: 33:10 note "'trivial_abi' is disallowed on 'D' because its copy constructors and move constructors are all deleted"
//@ diag: 34:10 warning "'trivial_abi' cannot be applied to 'E'"
//@ diag: 34:10 note "'trivial_abi' is disallowed on 'E' because it is polymorphic"
//@ diag: 35:10 warning "'trivial_abi' cannot be applied to 'F'"
//@ diag: 35:10 note "'trivial_abi' is disallowed on 'F' because it has a virtual base"
//@ diag: 36:10 warning "'trivial_abi' cannot be applied to 'G'"
//@ diag: 36:10 note "'trivial_abi' is disallowed on 'G' because it has a field of a non-trivial class type"
//@ diag: 37:10 warning "'trivial_abi' cannot be applied to 'H'"
//@ diag: 37:10 note "'trivial_abi' is disallowed on 'H' because it has a base of a non-trivial class type"
//@ diag: 38:10 warning "'trivial_abi' cannot be applied to 'J'"
//@ diag: 38:10 note "'trivial_abi' is disallowed on 'J' because it is polymorphic"
//@ diag: 39:3 warning "trivial_abi' attribute only applies to classes"
//@ diag: 40:10 error "trivial_abi' attribute takes no arguments"
//@ diag: 41:23 error "'trivial_abi' attribute takes no arguments"
//@ diag: 44:10 warning "'trivial_abi' cannot be applied to 'R'"
//@ diag: 44:10 note "'trivial_abi' is disallowed on 'R' because its copy constructors and move constructors are all deleted"
struct N { N(const N &); };
struct [[clang::trivial_abi]] A { A(const A &); ~A(); long v; };
struct [[clang::trivial_abi]] D { D(const D &) = delete; D(D &&) = delete; ~D(); };
struct [[clang::trivial_abi]] E { virtual void f(); };
struct [[clang::trivial_abi]] F : virtual A {};
struct [[clang::trivial_abi]] G { N n; };
struct [[clang::trivial_abi]] H : N {};
struct [[clang::trivial_abi]] J : E {};
[[clang::trivial_abi]] void g();
struct [[clang::trivial_abi(1)]] I {};
struct __attribute__((trivial_abi(1))) I2 {};
template <class T> struct [[clang::trivial_abi]] T1 { T t; };
// Deleted copy and move: not trivially relocatable, so it is dropped.
struct [[clang::trivial_abi]] R { R(const R &) = delete; R(R &&) = delete; };
union [[clang::trivial_abi]] U { A a; long l; U(const U &); ~U(); };
struct B { A a; };
struct C { A a; ~C(); };
struct [[clang::trivial_abi]] P { P(P &&); ~P(); };
static_assert(__can_pass_in_regs(A));
static_assert(__can_pass_in_regs(B));
static_assert(!__can_pass_in_regs(C));
static_assert(!__can_pass_in_regs(D));
static_assert(!__can_pass_in_regs(E));
static_assert(!__can_pass_in_regs(J));
static_assert(!__can_pass_in_regs(R));
static_assert(__can_pass_in_regs(U));
static_assert(__can_pass_in_regs(P));
static_assert(__can_pass_in_regs(T1<A>));
static_assert(!__can_pass_in_regs(T1<E>));
// The dropped attribute leaves the implicit copy constructor declared at
// `}` trivial for calls (clang does the same).
static_assert(__can_pass_in_regs(G));
static_assert(__can_pass_in_regs(H));
static_assert(__can_pass_in_regs(T1<N>));
