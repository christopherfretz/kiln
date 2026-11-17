// [temp.local]/1-2: a base class template's injected-class-name named
// through the current instantiation with a dependent base, with and
// without template arguments.
//@ expect: compile-ok
//@ strict-diags
template <class T> struct Base { };
template <class T>
struct D3: Base<T> {
  typename D3::Base* p1;
  typename D3::template Base<double>* p2;
};
template struct D3<void>;
struct D1: Base<void> { D1::Base* p1; D1::Base<double>* p2; };
