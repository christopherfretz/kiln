// Where an explicit instantiation definition's entities are emitted
// (clang hands them to `HandleTopLevelDecl` at the explicit
// instantiation, or at the end of the TU when instantiated late), which
// also decides which callers are `nounwind` (clang `TryMarkNoThrow`).

// An explicit instantiation declaration followed by a definition: the
// members are defined at the definition, after their callers.
template <class T> struct V { void a() {} void b(); virtual void c() {} static T s; };
template <class T> void V<T>::b() {}
template <class T> T V<T>::s = 2;
extern template struct V<int>;
void uv(V<int> &x) { x.a(); x.b(); }
template struct V<int>;

// [temp.explicit]/3: a member class of a class template specialization;
// its member functions are defined there, member templates are not.
template <class T> struct O {
  struct I { void f(); int g() { return 2; } template <class U> void t() {} };
};
template <class T> void O<T>::I::f() {}
template void O<int>::I::f();
template struct O<long>::I;
extern template struct O<char>::I;
int uo(O<char>::I &i) { return i.g(); }

// A function template, member function or structor explicitly
// instantiated before its definition is instantiated at the end of the
// TU and defined after every top-level declaration, in order.
template <class T> T late(T);
template int late<int>(int);
template <class T> struct M { void f(); static T s; M(); ~M(); };
template void M<int>::f();
template int M<int>::s;
template M<int>::M();
template M<int>::~M();
int mid() { return 0; }
template <class T> T late(T t) { return t; }
template <class T> void M<T>::f() {}
template <class T> T M<T>::s = 4;
template <class T> M<T>::M() {}
template <class T> M<T>::~M() {}
template <class T> T late2(T);
template long late2<long>(long);
template <class T> T late2(T t) { return t + 1; }
int z() { return late(1) + M<int>::s; }
