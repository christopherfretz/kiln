//@ args: -std=c++26
// The visibility attribute of an explicit instantiation
// ([temp.explicit]; spec/codegen.md "Symbol visibility", spec/sema.md
// "Visibility attributes"; clang ActOnExplicitInstantiation processes its
// attributes on the specialization): it is the specialization's own, so it
// overrides a hidden function or variable template, both for an explicit
// instantiation declaration (libc++'s exported `extern template` function
// specializations) and for an explicit instantiation definition.
template <class T> __attribute__((visibility("hidden"))) T f(T);
extern template __attribute__((visibility("default"))) int f<int>(int);
template <class T> __attribute__((visibility("hidden"))) T f(T t) { return t; }

template <class T> T h(T t) { return t; }
template __attribute__((visibility("protected"))) long h<long>(long);
template __attribute__((visibility("hidden"))) short h<short>(short);

template <class T> __attribute__((visibility("hidden"))) T v = T();
extern template __attribute__((visibility("default"))) int v<int>;
template __attribute__((visibility("default"))) long v<long>;

int use() { return f(1) + v<int> + h(2); }
