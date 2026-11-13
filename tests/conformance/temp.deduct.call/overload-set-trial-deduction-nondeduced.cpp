// [temp.deduct.call]/6: an overload set argument whose trial deductions
// succeed for more than one member (/6.2), for none, or that contains a
// function template (/6.1) is a non-deduced context, so a parameter
// deduced nowhere else is "couldn't infer". A non-static member named
// `&N::get` is tried as a pointer to member function and so never
// deduces a pointer-to-function P. The one member that does deduce is
// the argument, which must agree with the other arguments' deductions.
// The set formed in a template is resolved at instantiation. Reduced
// from a downstream field-extraction header (extract_from over
// the overloaded getters of an internal namespace handles). Matches clang 21.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 45:3 error "no matching function for call to 'f'"
//@ diag: 30:39 note "candidate template ignored: couldn't infer template argument 'P'"
//@ diag: 46:3 error "no matching function for call to 'g'"
//@ diag: 31:27 note "candidate template ignored: couldn't infer template argument 'P'"
//@ diag: 47:3 error "no matching function for call to 'g'"
//@ diag: 31:27 note "candidate template ignored: couldn't infer template argument 'P'"
//@ diag: 48:3 error "no matching function for call to 'extract_from'"
//@ diag: 34:3 note "candidate template ignored: couldn't infer template argument 'P'"
//@ diag: 49:3 error "no matching function for call to 'g'"
//@ diag: 31:27 note "candidate template ignored: couldn't infer template argument 'P'"
//@ diag: 50:3 error "no matching function for call to 'h'"
//@ diag: 32:27 note "candidate template ignored: deduced conflicting types for parameter 'P' ('long' vs. 'int')"
//@ diag: 51:3 error "no matching function for call to 'g'"
//@ diag: 31:27 note "candidate template ignored: couldn't infer template argument 'P'"
//@ diag: 43:52 error "no matching function for call to 'extract_from'"
//@ diag: 52:3 note "in instantiation of function template specialization 'via<H2>' requested here"
//@ diag: 34:3 note "candidate template ignored: couldn't infer template argument 'P'"
template <typename P, typename S> int f(bool (S::*)(P *));
template <typename P> int g(bool (*)(P *));
template <typename P> int h(P *, bool (*)(P *));
template <typename P, typename T, typename S>
P extract_from(T const &o, bool (S::*ptr)(P *) const);
enum Side { Bid, Ask };
struct K { bool get(int *); bool get(long *); };
bool two(int *); bool two(long *);
template <typename T> bool tmix(T *); bool tmix(int *);
struct H2 { bool get(int *) const; bool get(Side *) const; int get() const; };
bool none(int); int none();
bool fr(int *); int fr();
struct N { bool get(int *); int get(); };
template <typename T> int via(T const &t) { return extract_from(t, &T::get); }
void t(H2 const &h2, long *l) {
  f(&K::get);
  g(&two);
  g(&tmix);
  extract_from(h2, &H2::get);
  g(&none);
  h(l, &fr);
  g(&N::get);
  via(h2);
}
