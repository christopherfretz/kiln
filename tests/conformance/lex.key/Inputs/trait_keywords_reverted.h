// The precompiled header of trait-keywords-pch.cpp: three revertible trait
// keywords reverted, as libstdc++'s <bits/cpp_type_traits.h> does.
template <class T> struct __is_arithmetic { enum { __value = 1 }; };
struct __is_void { int x; };
struct N { static const bool __is_signed = true; };
