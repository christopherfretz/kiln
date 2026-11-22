// The written-reference table (spec/ast.md §18.4), role `template`: the
// template name of a template-id in a type context ([temp.names]),
// unqualified and qualified, a class template argument deduction
// placeholder ([dcl.type.class.deduct]), an alias template, a template
// template argument and parameter default ([temp.arg.template]), a member
// template after `.template` and `::template`, and a template found
// through a using-declaration (`found` is the using-shadow). The type
// name of a template-id follows its template arguments' entries.
//@ written-refs: template-names.refs
namespace lib {
template <class T> struct Vec {
  T* data;
  template <class U> U as() const { return U(); }
  template <class U> struct Rebind { using type = Vec<U>; };
};
template <class A, class B> struct Pair {
  A a;
  B b;
};
template <class T> Pair(T, T) -> Pair<T, T>;
template <class T> using Ptr = T*;
} // namespace lib

using lib::Pair;

template <template <class> class C = lib::Vec> struct Holder {
  C<int> c;
};

namespace app {
lib::Vec<int> numbers;
Pair<int, long> two;
lib::Ptr<lib::Vec<char>> ptr;
Holder<lib::Vec> held;
Holder<> defaulted;

int use() {
  lib::Pair p{1, 2};           // CTAD, qualified
  Pair q{3, 4};                // CTAD through the using-declaration
  typename lib::Vec<int>::template Rebind<long>::type r{};
  return numbers.template as<int>() + p.a + q.b + int(r.data == nullptr);
}

template <class T> int dep(lib::Vec<T>& v) {
  typename lib::Vec<T>::template Rebind<T>::type w{};
  return v.template as<int>() + w.template as<int>();
}
} // namespace app
