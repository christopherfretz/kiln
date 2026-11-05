// [dcl.type.decltype]/2 (C++11, N3276): when the operand of decltype is a
// function call returning a class prvalue, no temporary is introduced, so
// the return type may be incomplete or have a deleted destructor.
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
struct Inc;
Inc make();
typedef decltype(make()) T;
T* p = nullptr;
struct NoDtor { ~NoDtor() = delete; };
NoDtor nd();
typedef decltype(nd()) U;
template <class X> struct Never { static_assert(sizeof(X) == 0, "instantiated"); };
Never<int> never();
typedef decltype(never()) V;
