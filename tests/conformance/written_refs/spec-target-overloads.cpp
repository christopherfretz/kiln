// The written-reference table (spec/ast.md §18.4): the `spec-target`
// entry of an explicit specialization ([temp.expl.spec]/1) or explicit
// instantiation ([temp.explicit]/2) written with a template-id names
// the function template the declaration matched by deduction
// ([temp.deduct.decl]), not the first that lookup of the name found:
// here the second of two overloaded templates `f`, and the first for
// the declarations whose type matches it.
//@ written-refs: spec-target-overloads.refs
namespace n {
template <class T> void f(T) {}
template <class T> void f(T, int) {}
template <class T> constexpr int v = 0;
} // namespace n

template <> void n::f<>(char, int) {}
template <> void n::f<>(char) {}
template void n::f<>(short, int);
template void n::f<short>(short);
extern template void n::f<long>(long, int);
template <> constexpr int n::v<char> = 1;
