//@ expect: ast
//@ oracle: clang
// [temp.names]/1, [temp.arg.explicit]: template-ids naming function templates
template <class T> void foo(T);
template <class T, class U> void bar(U);
template <class T> void call(T t) { foo<T>(t); bar<T>(t); foo(t); }
