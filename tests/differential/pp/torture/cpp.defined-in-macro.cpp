//@ expect: pp
// `defined` produced by macro expansion in #if: clang evaluates it (with a
// -Wexpansion-to-defined warning under -pedantic), as GCC does.
#define X
#define HAS_X defined(X)
#define HAS_Y defined Y
// The parameter is an operand of ##, so the argument is not pre-expanded
// (X itself expands to nothing).
#define HAS(m) defined(m ## _FLAG)
#define X_FLAG
#if HAS_X
has_x;
#endif
#if HAS_Y
has_y;
#else
no_y;
#endif
#if HAS(X) && !HAS(Z)
has_x_not_z;
#endif
// In ordinary text, `defined` is just an identifier.
HAS_X HAS_Y HAS(foo) defined
#if defined X && defined(X) && defined ( X )
all_forms;
#endif
