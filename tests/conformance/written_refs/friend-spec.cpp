// The written-reference table (spec/ast.md §18.4, §18.9, §18.12): a
// friend function template specialization ([temp.friend]/1.3) names the
// template whose specialization it befriends. Outside a template, and
// in one when it depends on nothing of the template (`f<int>(int)`),
// that is the template its declaration matched, not the first template
// lookup found. In a template with dependent arguments or type
// (`f<>(T)`), the match waits for each specialization of the class: the
// entry is dependent, its found set the templates lookup found (closed:
// no argument-dependent lookup), and each instantiation adds a row
// naming the specialization befriended, found as its template. `args=`
// is the template argument list's `<` and `>` (§18.11.2). A class
// template never instantiated (`Never`) keeps the pattern entry alone.
//@ written-refs: friend-spec.refs
namespace n {
template <class T> void f(T) {}
template <class T> void f(T, int) {}

template <class T> class A {
  friend void f<>(T);
  friend void f<>(T, int);
};
template <class T> class B {
  friend void f<int>(int, int);
  friend void n::f<T>(T);
};
template <class T> class Never {
  friend void f<T>(T, int);
};
} // namespace n

struct Q {
  friend void n::f<>(long, int);
  friend void n::f<long>(long);
};

void run() {
  n::A<int> a;
  n::A<char> c;
  n::B<short> b;
}
