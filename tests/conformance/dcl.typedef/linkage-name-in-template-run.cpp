// [dcl.typedef]/9: an unnamed class or enumeration defined in a typedef
// declaration takes the first typedef-name declared by it for linkage
// purposes, and the Itanium ABI mangles it by that name. A member of a
// class template is no exception: the instantiated tag takes the
// instantiated typedef's name, as the pattern's did (clang
// `TemplateDeclInstantiator::VisitTypedefNameDecl`), so `S<false>::
// Direction` is `N1SILb0EE9DirectionE`, not the unnamed type `NS0_Ut_E`.
// The members below are only declared here; the other unit
// (Inputs/linkage-name-in-template-defs.cpp) defines each under the name
// clang and GCC give it, with an asm label, so the program links only if
// the compiler's references carry those names. A dependency's
// `MultiStreamIterator<IS_CONST>` declares `typedef enum { k_Forward = 1,
// k_Backward = 2 } Direction;` and its constructor taking a Direction is
// defined in a GCC-built library.
//@ expect: run
//@ args: -Wno-undefined-var-template %{test_dir}/Inputs/linkage-name-in-template-defs.cpp
//@ stdout: f 21 g 3 mt 11
//@ stdout: In 55 MT 6 P 79 Q 8 r 2 vt 9
extern "C" int printf(const char *, ...);

template <bool B> struct S {
  typedef enum { k_Forward = 1, k_Backward = 2 } Direction;
  typedef enum { e_Begin, e_End } Position;
  typedef struct { int v; } Rec;
  typedef union { int i; float f; } Un;
  static int f(Direction, Position);
  static int g(Rec *, Un *);
  template <class U> static int mt(U, Direction);  // a member template
  template <int N> struct In {                     // a nested template
    typedef enum { i_A = 5 } E;
    typedef struct { int w; } R;
    static int h(E, R *);
  };
  template <class U> struct MT {
    typedef enum { q_A = 6 } QE;
    static int k(QE);
  };
};
template <class T> struct P;
template <class T> struct P<T *> {                 // a partial specialization
  typedef enum { p_A = 7 } PE;
  typedef struct { int z; } PR;
  static int m(PE, PR *);
};
template <class T> struct Q {
  typedef enum { q_B = 8 } E, E2;                  // the first name, E, is it
  static int n(E2);
};
template <class T> typename S<false>::Direction r(T);  // in a return type
template <class T> extern int vt;                       // in a variable's name

using D = S<false>;

int main() {
  D::Rec rec{1};
  D::Un un{2};
  D::In<3>::R ir{5};
  P<int *>::PR pr{9};
  printf("f %d g %d mt %d\n", D::f(D::k_Backward, D::e_End), D::g(&rec, &un),
         D::mt(1, D::k_Forward));
  printf("In %d MT %d P %d Q %d r %d vt %d\n", D::In<3>::h(D::In<3>::i_A, &ir),
         D::MT<int>::k(D::MT<int>::q_A), P<int *>::m(P<int *>::p_A, &pr),
         Q<int>::n(Q<int>::q_B), (int)r(0), vt<D::Direction>);
}
