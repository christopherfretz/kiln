//@ verify
// GCC vector modes (clang `handleModeAttr`): `V<N>` needs N a power of
// two (otherwise the whole name is the mode); each one is deprecated; an
// enumeration type (or declaration) takes no vector mode, an enumeration
// declaration no floating-point mode, and a vector type no complex mode.
typedef int m __attribute__((mode(V3SI))); // expected-error {{unknown machine mode 'V3SI'}}
typedef int n __attribute__((mode(V0SI))); // expected-error {{unknown machine mode 'V0SI'}}
typedef int o __attribute__((mode(V2XX))); // expected-warning {{specifying vector types with the 'mode' attribute is deprecated; use the 'vector_size' attribute instead}} expected-error {{unknown machine mode 'V2XX'}}
typedef float q __attribute__((mode(V4SI))); // expected-warning {{deprecated}} expected-error {{type of machine mode does not match type of base type}}
enum E { X };
typedef E r __attribute__((mode(V4SI))); // expected-warning {{deprecated}} expected-error {{mode 'V4SI' is not supported for enumeration types}}
enum K { KX } __attribute__((mode(SF))); // expected-error {{type of machine mode does not match type of base type}}
enum L { LX } __attribute__((mode(V4SI))); // expected-warning {{deprecated}} expected-error {{mode 'V4SI' is not supported for enumeration types}}
typedef int v4si __attribute__((vector_size(16)));
typedef v4si s __attribute__((mode(SC))); // expected-error {{type of machine mode does not match type of base type}} expected-error {{type of machine mode does not support base vector types}}
static_assert(sizeof(m) == sizeof(int));
