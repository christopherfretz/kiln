// [temp.constr.atomic]/3: the rejected side of
// concept-id-pack-concept-argument-substitution-failure.cpp. With no
// fallback the call has no viable candidate; the failed substitution is
// reported at the concept-id's argument.
//@ args: -std=c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:9 error "no matching function for call to 's'"
//@ diag: 14:69 note "candidate template ignored: constraints not satisfied [with R = int *&]"
//@ diag: 14:54 note "because substituted constraint expression is ill-formed: member reference base type 'int *' is not a structure or union"
template <class T> T&& declval();
template <class T, class... A> concept cf = __is_constructible(T, A...);
template <class T> concept mc = cf<T, T>;
template <class R> requires mc<decltype(declval<R&>().begin())> int s(R&&);
int* p;
int x = s(p);
