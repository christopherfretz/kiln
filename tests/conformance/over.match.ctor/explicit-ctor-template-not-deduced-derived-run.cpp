// [over.match.ctor]/1, [over.match.copy]/1.1, [dcl.init.general]/16.6.2:
// copy-initializing a class from itself or from a derived class goes
// through its constructors, but only the converting ones are candidates,
// so a non-dependently explicit constructor template is not deduced at
// all (clang ResolveConstructorOverload -> AddTemplateOverloadCandidate
// without AllowExplicit). Here deducing it would substitute a default
// template argument that checks the concept being satisfied, a
// self-dependent constraint that is a hard error ([temp.constr.atomic]/3);
// the copy constructor is selected instead. The same holds for
// copy-initialization from a non-class type. Reduced from a value-or-error
// converting constructor in Boost.Outcome (WP-1223).
//@ expect: run
//@ strict-diags
//@ stdout: copy copy copy int 1 1
extern "C" int printf(const char*, ...);
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
template <class X, class> struct first { using type = X; };
template <class T> struct R;
template <class U>
concept is_r = requires { typename U::value_type; } &&
               __is_convertible(U, R<typename U::value_type>);
template <class U> concept from_int = requires { typename U::value_type; } &&
               __is_convertible(int, U);
template <class T> struct R {
  using value_type = T;
  R() {}
  R(const R&) { printf("copy "); }
  R(int) { printf("int "); }
  template <class U, typename = typename en<!is_r<U>>::type>
  explicit R(U&&) { printf("template "); }
  template <class U, typename = typename en<!from_int<typename first<R, U>::type>>::type>
  explicit R(U, int = 0) { printf("template "); }
};
struct D : R<int> {};
void take(R<int>) {}
static_assert(is_r<D>);
static_assert(from_int<R<int>>);
int main() {
  D d;
  R<int> a = d;
  take(d);
  R<int> b = static_cast<const D&>(d);
  R<int> c = 3;
  printf("%d %d\n", (int)is_r<D>, (int)from_int<R<int>>);
  (void)a;
  (void)b;
  (void)c;
}
