// [temp.over.link]/5: member access and casts on dependent operands.
struct S { int m; };
template <class T> auto f(T t) -> decltype(t.m + static_cast<long>(sizeof(T)));
template <class T> auto f(T u) -> decltype(u.m + static_cast<long>(sizeof(T))) { return 0; }
long r = f(S());
