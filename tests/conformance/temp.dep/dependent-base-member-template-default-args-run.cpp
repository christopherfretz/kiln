// [temp.dep.type]/7, [temp.inst]/12, [dcl.fct.default]/1, [expr.ref]: a
// static member function found at instantiation through a dependent base
// by `this->f(...)` is called with the default arguments of its omitted
// trailing parameters, as a call by its name is: a member function
// template of a partial specialization whose defaulted parameter's type
// depends on the deduced arguments (a SFINAE guard), a non-template
// static member, a qualified call, and a defaulted non-dependent
// parameter of a member template. Run through codegen; each call reports
// the value its default argument supplied.
//@ expect: run
//@ strict-diags
//@ stdout: 7 7 11 3 4 9 42 5
extern "C" int printf(const char*, ...);

template <bool C, class R = void> struct only_if {};
template <class R> struct only_if<true, R> { using type = R; };
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
template <class A> struct unconst { using type = A; };
template <class A> struct unconst<const A> { using type = A; };

template <class T> struct trivial { static constexpr bool value = true; };
template <> struct trivial<char> { static constexpr bool value = false; };

template <class T, bool Fast> struct Mover;
template <class T> struct Mover<T, false> {
  template <class In, class Out> static int move(In, In, Out) { return -1; }
};
template <class T> struct Mover<T, true> {
  template <class S, class D>
  static int move(S* first, S* last, D* out,
                  typename only_if<same<typename unconst<S>::type, D>::value>::type* g = nullptr,
                  int tag = 7) {
    (void)g;
    int n = 0;
    for (; first != last; ++first, ++out, ++n) *out = *first;
    return n == 3 ? tag : -n;
  }
};
template <class T> struct Vec : Mover<T, trivial<T>::value> {
  T data[3] = {1, 2, 3};
  int copy_to(Vec& other) { return this->move(data, data + 3, other.data); }
  int copy_from_const(const T* src) { return this->move(src, src + 3, data); }
};

// (a) a primary template only, the defaulted parameter dependent.
template <class T> struct Plain {
  template <class U> static int pick(U*, int k = 11) { return k; }
};
template <class T> struct UsePlain : Plain<T> {
  int run(T* p) { return this->pick(p); }
};

// (b) a non-template static member with a defaulted parameter.
template <class T> struct Stat {
  static int add(int a, int b = 2) { return a + b; }
};
template <class T> struct UseStat : Stat<T> {
  int run() { return this->add(1); }
  // (c) the same, named by a qualified-id.
  int qual() { return Stat<T>::add(2); }
};

// (d) a member template's defaulted non-dependent parameter.
template <class T> struct Tmpl {
  template <class U> static int scale(U u, int by = 3) { return u * by; }
};
template <class T> struct UseTmpl : Tmpl<T> {
  int run(T t) { return this->scale(t); }
};

// A non-static member function template keeps its default too.
template <class T> struct NonStatic {
  template <class U> int get(U u, int plus = 37) { return u + plus; }
};
template <class T> struct UseNonStatic : NonStatic<T> {
  int run(T t) { return this->get(t); }
};

// An object expression other than `this`.
template <class T> int via_object(Stat<T>& s, T v) { return s.add(v); }

int main() {
  Vec<int> a, b;
  const int src[3] = {4, 5, 6};
  int x = 0;
  UsePlain<int> p;
  UseStat<int> s;
  UseTmpl<int> t;
  UseNonStatic<int> n;
  Stat<int> st;
  printf("%d %d %d %d %d %d %d %d\n", a.copy_to(b), b.copy_from_const(src), p.run(&x), s.run(),
         s.qual(), t.run(3), n.run(5), via_object(st, 3));
  return b.data[2] == 6 ? 0 : 1;
}
