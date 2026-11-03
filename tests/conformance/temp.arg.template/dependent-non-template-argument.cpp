// [temp.arg.template]/1, [temp.res.general]/6: an argument for a template
// template parameter must name a class template or alias template; a
// specialization, a type or a value is rejected when the enclosing
// template is defined, even with dependent arguments
// (spec/sema-templates.md §4.7).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:5 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 25:5 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 26:5 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 27:5 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 28:9 error "template argument for template template parameter must be a class template or type alias template"
//@ diag: 29:5 error "template argument for template template parameter must be a class template or type alias template"
template <template <class> class TT> struct H {};
template <template <class> class... TT> struct HP {};
struct W {
  template <class> struct X {};
};
template <class T> struct Y {
  H<Y> ok; // the injected-class-name names the template
  H<Y<T>> bad;
};
template <class T, int N, class... Ts> struct Z {
  H<T::template X> ok;
  H<Y<T>> bad1;
  H<T> bad2;
  H<N> bad3;
  HP<Y, T> bad4;
  H<T::X> bad5;
};
