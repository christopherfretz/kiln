// [meta.reflection.names]/8 (P2996R13): source_location_of's file name
// is implementation-defined; under -fkiln-source-path-style=gcc kiln gives
// GCC 16.1's (spec/reflection.md §3.9), compared with GCC (spec/harness.md
// "GCC oracle"). GCC names a header by the path its search built (libcpp
// append_file_to_dir): the -I / -iquote directory as given, a `/` unless it
// ends in one, then the header name; a header included relative to its
// includer takes the includer's name up to its last `/`. Nothing is
// normalized: `-I /p//Inputs/../Inputs/./` names the header
// `/p//Inputs/../Inputs/./slo-header.h`, a doubled trailing slash stays.
// A header found through a system directory (-isystem) is named by its
// realpath when that is shorter (-fcanonical-system-headers, GCC's
// default), and so is one included relative to it, or to a header that
// made itself a system header with `#pragma GCC system_header` before its
// directory was first searched (libcpp caches the answer per directory).
//@ args: -freflection -fkiln-source-path-style=gcc --gcc-toolchain=%{gcc_toolchain} -I %{test_dir}//Inputs/../Inputs/./ -I %{test_dir}/Inputs/slo-path// -iquote %{test_dir}/Inputs/./slo-quote/ -isystem %{test_dir}//Inputs/slo-sys/../slo-sys/./ -I %{test_dir}/Inputs/./slo-prag/
//@ oracle: gcc
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include <meta>
#include <slo-header.h>
#include <slo-trail.h>
#include "slo-quote.h"
#include <slo-sys.h>
#include <slo-prag.h>
using namespace std::meta;

consteval bool ends_with(std::string_view s, std::string_view tail) { return s.ends_with(tail); }
consteval bool canonical(std::string_view s) {
  return s.starts_with('/') && !s.contains("//") && !s.contains("/./") && !s.contains("/../");
}
consteval std::string_view file_of(info r) { return source_location_of(r).file_name(); }

// -I with empty, `.` and `..` components: kept as written.
static_assert(ends_with(file_of(^^InHeader), "/meta.reflection.names//Inputs/../Inputs/./slo-header.h"));
static_assert(source_location_of(^^InHeader).line() == 3);
// A doubled trailing slash: no separator is added after it.
static_assert(ends_with(file_of(^^InTrail), "/meta.reflection.names/Inputs/slo-path//slo-trail.h"));
// `#include "../slo-rel.h"` from it: the includer's directory part.
static_assert(ends_with(file_of(^^InRel), "/meta.reflection.names/Inputs/slo-path//../slo-rel.h"));
// -iquote, with a single trailing slash.
static_assert(ends_with(file_of(^^InQuote), "/meta.reflection.names/Inputs/./slo-quote/slo-quote.h"));
// -isystem: the realpath, and a relative include from it too.
static_assert(canonical(file_of(^^InSys)));
static_assert(ends_with(file_of(^^InSys), "/meta.reflection.names/Inputs/slo-sys/slo-sys.h"));
static_assert(canonical(file_of(^^InSysRel)));
static_assert(ends_with(file_of(^^InSysRel), "/meta.reflection.names/Inputs/slo-sys-rel.h"));
// A user header itself, then system after its pragma.
static_assert(ends_with(file_of(^^InPrag), "/meta.reflection.names/Inputs/./slo-prag/slo-prag.h"));
static_assert(canonical(file_of(^^InPragRel)));
static_assert(ends_with(file_of(^^InPragRel), "/meta.reflection.names/Inputs/slo-prag-rel.h"));
// One translation unit is self-consistent: __builtin_FILE() agrees.
static_assert(file_of(^^InHeader) == std::string_view(InHeaderFile));
static_assert(file_of(^^InTrail) == std::string_view(InTrailFile));
static_assert(file_of(^^InRel) == std::string_view(InRelFile));
static_assert(file_of(^^InQuote) == std::string_view(InQuoteFile));
static_assert(file_of(^^InSys) == std::string_view(InSysFile));
static_assert(file_of(^^InSysRel) == std::string_view(InSysRelFile));
static_assert(file_of(^^InPrag) == std::string_view(InPragFile));
static_assert(file_of(^^InPragRel) == std::string_view(InPragRelFile));

int main() {}
