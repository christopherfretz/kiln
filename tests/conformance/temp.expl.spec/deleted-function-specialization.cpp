// [temp.expl.spec]/1, [dcl.fct.def.delete]/4: an explicit specialization of a
// function template may be a deleted definition; it is the specialization's
// first declaration (the template's declaration is not a previous one), and
// a call to the specialization uses a deleted function.
//@ expect: compile-ok
template <class T> constexpr int f() { return 0; }
template <> constexpr int f<short>() = delete;
template <> constexpr int f<long>() = delete("no long");
template <class T>
concept Callable = requires { f<T>(); };
static_assert(Callable<int> && !Callable<short> && !Callable<long>);
