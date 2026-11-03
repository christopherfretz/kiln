// [temp.inst]/12 (WP-156): a default argument whose instantiation failed
// is instantiated, and diagnosed, again at each later use, as clang does;
// a function template's default argument is instantiated at its use too.
//@ expect: compile-fail
//@ diag: 28:40 error "no viable conversion from 'const int' to 'B'"
//@ diag: 33:13 note "in instantiation of default function argument expression for 'f<B>' required here"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'const int' to 'const B &' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'const int' to 'B &&' for 1st argument"
//@ diag: 28:40 note "passing argument to parameter 'x' here"
//@ diag: 28:40 error "no viable conversion from 'const int' to 'B'"
//@ diag: 33:20 note "in instantiation of default function argument expression for 'f<B>' required here"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'const int' to 'const B &' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'const int' to 'B &&' for 1st argument"
//@ diag: 28:40 note "passing argument to parameter 'x' here"
//@ diag: 28:79 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:3 note "in instantiation of default function argument expression for 's<int>' required here"
//@ diag: 28:79 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:16 note "in instantiation of default function argument expression for 's<int>' required here"
//@ diag: 30:30 error "no viable conversion from 'const int' to 'V'"
//@ diag: 35:3 note "in instantiation of default function argument expression for 'ft<V>' required here"
//@ diag: 31:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'const int' to 'const V &' for 1st argument"
//@ diag: 31:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'const int' to 'V &&' for 1st argument"
//@ diag: 30:30 note "passing argument to parameter 'x' here"
//@ diag: 30:37 error "no member named 'v' in 'B'"
//@ diag: 35:12 note "in instantiation of default function argument expression for 'ft<B>' required here"
//@ requires: clang>=22
// clang 22 spelling of lazy-default-args-retry.cpp (WP-394).
template <class T> struct A { void f(T x = T::value) {} static void s(int y = T::nope) {} };
struct B { static const int value = 3; };
template <class T> void ft(T x = T::v) {}
struct V { static const int v = 1; };
void g() {
  A<B> b; b.f(); b.f();
  A<int>::s(); A<int>::s();
  ft<V>(); ft<B>();
}
