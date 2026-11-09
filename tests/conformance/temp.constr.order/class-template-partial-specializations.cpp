// [temp.constr.order], [temp.spec.partial.general]/9.2: a partial
// specialization equally specialized as the primary template must be more
// constrained; partial specializations of a class template and of a member
// class template of a class template are ordered by their constraints.
// Ported from clang test/CXX/temp/temp.constr/temp.constr.order/
// class-template-partial-specializations.cpp (llvmorg-19.1.0), less the
// `E` case (kiln reports a redefinition where clang reports "requires clause
// differs in template redeclaration": follow-up). WP-1053.
//@ std: c++20
//@ verify

template<typename T> requires (sizeof(T) >= 4)
// expected-note@-1{{similar constraint expressions not considered equivalen}}
class A{}; // expected-note{{template is declared here}}

template<typename T> requires (sizeof(T) >= 4 && sizeof(T) <= 10)
// expected-note@-1{{similar constraint expression here}}

class A<T>{}; // expected-error{{class template partial specialization is not more specialized than the primary template}}

template<typename T>
concept C1 = sizeof(T) >= 4;

template<typename T> requires C1<T>
class B{};

template<typename T> requires (C1<T> && sizeof(T) <= 10)
class B<T>{};

template<typename T>
concept C2 = sizeof(T) > 1 && sizeof(T) <= 8;

template<typename T>
class C{};

template<typename T> requires C1<T>
class C<T>{};

template<typename T>
class D{}; // expected-note{{previous definition is here}}

template<typename T>
class D<T>{}; // expected-error{{class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list}} expected-error{{redefinition of 'D'}}

template<typename T>
struct F{ enum{ value = 1 }; };

template<typename T> requires C1<T> && C2<T>
struct F<T>{ enum{ value = 2 }; };

template<typename T> requires C1<T> || C2<T>
struct F<T>{ enum{ value = 3 }; };

static_assert(F<unsigned>::value == 2);
static_assert(F<char[10]>::value == 3);
static_assert(F<char>::value == 1);

template <unsigned I>
struct S {
  template <typename T>
  struct F {
    enum { value = 1 };
  };

  template <typename T>
    requires C1<T> && C2<T>
  struct F<T> {
    enum { value = 2 };
  };

  template <typename T>
    requires C1<T> || C2<T>
  struct F<T> {
    enum { value = 3 };
  };
};

static_assert(S<1>::F<unsigned>::value == 2);
static_assert(S<1>::F<char[10]>::value == 3);
static_assert(S<1>::F<char>::value == 1);

// Make sure atomic constraints subsume each other only if their parameter
// mappings are identical.

template<typename T, typename U> requires C2<T>
struct I { }; // expected-note {{template is declared here}}

template<typename T, typename U> requires C2<U>
struct I<T, U> { }; // expected-error {{class template partial specialization is not more specialized than the primary template}}

template<typename T, typename U> requires C2<T> && C2<U>
struct I<T, U> { };
