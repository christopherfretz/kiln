/*===---- __stdarg_header_macro.h ------------------------------------------===
 *
 * Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
 * See https://llvm.org/LICENSE.txt for license information.
 * SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 *
 *===-----------------------------------------------------------------------===
 */

#ifndef __STDARG_H
#define __STDARG_H
#endif

/* C23 7.16/1 and C++26 [cstdarg.syn] (kiln). */
#if ((defined(__STDC_VERSION__) && __STDC_VERSION__ >= 202311L) ||            \
     (defined(__cplusplus) && __cplusplus > 202302L)) &&                       \
    !defined(__STDC_VERSION_STDARG_H__)
#define __STDC_VERSION_STDARG_H__ 202311L
#endif
