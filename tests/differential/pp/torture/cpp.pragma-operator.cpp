//@ expect: pp
// [cpp.pragma.op]: _Pragma("...") is destringized and becomes a #pragma
// line in the output, also when it comes from a macro expansion or sits in
// a macro argument.
#define DO_PRAGMA(x) _Pragma(#x)
#define ID(x) x
before1 _Pragma("kiln_torture plain") after1
before2 DO_PRAGMA(kiln_torture "from a macro" (1, 2)) after2
before3 ID(_Pragma("kiln_torture in argument") arg) after3
_Pragma("GCC diagnostic push")
_Pragma("GCC diagnostic ignored \"-Wunused\"")
_Pragma("GCC diagnostic pop")
_Pragma(L"kiln_torture wide")
_Pragma( "kiln_torture spaced" )
#define WITH_ESCAPES _Pragma("kiln_torture \"quoted\" \\ backslash")
WITH_ESCAPES
#pragma kiln_torture direct  spacing   kept
