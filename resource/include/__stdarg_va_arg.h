/*===---- __stdarg_va_arg.h - Definitions of va_start, va_arg, va_end-------===
 *
 * Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
 * See https://llvm.org/LICENSE.txt for license information.
 * SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 *
 *===-----------------------------------------------------------------------===
 */

#ifndef va_arg

#if (defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L) ||             \
    (defined(__cplusplus) && __cplusplus > 202302L)
/* C23 7.16.1.4 and C++26 [cstdarg.syn] (kiln): va_start(ap, ...) needs no
 * parameter name, and arguments after the first are neither expanded nor
 * evaluated: only their spelling reaches the builtin, which warns
 * (-Wvarargs) unless it is the last named parameter. */
#define va_start(ap, ...)                                                      \
  __builtin_va_start(ap, 0 __VA_OPT__(, #__VA_ARGS__))
#else
/* Versions before C23 do require the second parameter. */
#define va_start(ap, param) __builtin_va_start(ap, param)
#endif
#define va_end(ap) __builtin_va_end(ap)
#define va_arg(ap, type) __builtin_va_arg(ap, type)

#endif
