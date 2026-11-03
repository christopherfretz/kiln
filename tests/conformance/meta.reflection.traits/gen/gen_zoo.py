#!/usr/bin/env python3
"""Writes the [meta.reflection.traits] type-zoo tests (WP-477).

  traits-zoo.cpp      kiln only (-freflection): is_X_type(^^T) == std::is_X_v<T>
                      (and X(^^T) == ^^std::X_t<T> for the transformations),
                      for every zoo type and every trait with a <type_traits>
                      counterpart.
  traits-zoo-rhs.cpp  both compilers (min-oracle 22): std::is_X_v<T> == V, with
                      V computed here by compiling and running a probe with
                      clang++-22, so the std:: side the zoo compares against is
                      clang 22's answer.

Run from the repository root: python3 tests/conformance/meta.reflection.traits/gen/gen_zoo.py
(needs clang++-22 on PATH; --clang to name another). Commit the outputs.
"""

import argparse
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.dirname(HERE)

# Declarations the zoo types use (valid C++ for both compilers).
PRELUDE = """\
struct Empty {};
struct Agg { int a; double b; };
struct Agg2 { int x; double y; };
struct B {};
struct Der : B {};
struct VDer : virtual B {};
struct Poly { virtual void f(); };
struct Abs { virtual void f() = 0; };
struct Fin final { int a; };
union U { int a; float b; };
struct NoCopy { NoCopy() = default; NoCopy(const NoCopy&) = delete; };
struct MoveOnly { MoveOnly() = default; MoveOnly(MoveOnly&&) = default; };
struct Throws { Throws(); Throws(const Throws&) noexcept(false); Throws& operator=(const Throws&); };
struct NtDtor { ~NtDtor() noexcept(false); };
struct UserDtor { ~UserDtor(); };
struct NoDef { NoDef(int); };
struct Conv { operator int() const; };
enum E { e0 };
enum class SE : unsigned char { a };
"""

# The unary zoo: (alias name, type-id).
UNARY = [
    ("Void", "void"), ("CVoid", "const void"), ("Int", "int"), ("CInt", "const int"),
    ("VInt", "volatile int"), ("Bool", "bool"), ("Char", "char"), ("UInt", "unsigned"),
    ("LDouble", "long double"), ("Float", "float"), ("Nullptr", "decltype(nullptr)"),
    ("IntP", "int*"), ("CIntP", "const int*"), ("IntPC", "int* const"), ("IntL", "int&"),
    ("CIntL", "const int&"), ("IntR", "int&&"), ("Arr3", "int[3]"), ("ArrU", "int[]"),
    ("Arr23", "int[2][3]"), ("Fn", "int()"), ("FnNe", "int(int) noexcept"),
    ("FnC", "int() const"), ("FnP", "int(*)()"), ("FnL", "int(&)()"), ("MemP", "int Agg::*"),
    ("MemFnP", "int (Agg::*)()"), ("En", "E"), ("SEn", "SE"), ("TEmpty", "Empty"),
    ("TAgg", "Agg"), ("TB", "B"), ("TDer", "Der"), ("TVDer", "VDer"), ("TPoly", "Poly"),
    ("TAbs", "Abs"), ("TFin", "Fin"), ("TU", "U"), ("TNoCopy", "NoCopy"),
    ("TMoveOnly", "MoveOnly"), ("TThrows", "Throws"), ("TNtDtor", "NtDtor"),
    ("TUserDtor", "UserDtor"), ("TNoDef", "NoDef"), ("TConv", "Conv"), ("CAgg", "const Agg"),
    ("AggArr", "Agg[2]"),
]

# The binary zoo (every ordered pair).
BINARY = ["Int", "CInt", "Long", "IntL", "CIntL", "IntR", "IntP", "Void", "TB", "TDer",
          "TVDer", "TAgg", "TAgg2", "TConv", "LongCL"]
EXTRA = [("Long", "long"), ("TAgg2", "Agg2"), ("LongCL", "const long&")]

UNARY_BOOL = [
    "void", "null_pointer", "integral", "floating_point", "array", "pointer",
    "lvalue_reference", "rvalue_reference", "member_object_pointer",
    "member_function_pointer", "enum", "union", "class", "function", "reference",
    "arithmetic", "fundamental", "object", "scalar", "compound", "member_pointer", "const",
    "volatile", "trivially_copyable", "standard_layout", "empty", "polymorphic",
    "abstract", "final", "aggregate", "signed", "unsigned", "bounded_array",
    "unbounded_array", "scoped_enum", "default_constructible", "copy_constructible",
    "move_constructible", "copy_assignable", "move_assignable", "swappable",
    "destructible", "trivially_default_constructible", "trivially_copy_constructible",
    "trivially_move_constructible", "trivially_copy_assignable",
    "trivially_move_assignable", "trivially_destructible",
    "nothrow_default_constructible", "nothrow_copy_constructible",
    "nothrow_move_constructible", "nothrow_copy_assignable", "nothrow_move_assignable",
    "nothrow_swappable", "nothrow_destructible",
]
# Traits not named is_X: (reflection function, std trait).
UNARY_HAS = [("has_virtual_destructor", "has_virtual_destructor"),
             ("has_unique_object_representations", "has_unique_object_representations")]
# Size-valued queries: (reflection call, std value), {} the alias.
UNARY_SIZE = [("rank(^^{})", "std::rank_v<{}>"), ("extent(^^{})", "std::extent_v<{}>"),
              ("extent(^^{}, 1)", "std::extent_v<{}, 1>")]
TRANSFORMS = ["remove_const", "remove_volatile", "remove_cv", "add_const", "add_volatile",
              "add_cv", "remove_reference", "add_lvalue_reference", "add_rvalue_reference",
              "remove_extent", "remove_all_extents", "remove_pointer", "add_pointer",
              "remove_cvref", "decay"]
# is_X_type(^^A, ^^B) == std::is_X_v<A, B>.
BINARY_BOOL = ["same", "base_of", "convertible", "nothrow_convertible", "assignable",
               "trivially_assignable", "nothrow_assignable", "layout_compatible",
               "pointer_interconvertible_base_of", "swappable_with",
               "nothrow_swappable_with"]
BINARY_PLAIN = ["reference_constructs_from_temporary", "reference_converts_from_temporary"]
# is_X_type(^^A, {^^B}) == std::is_X_v<A, B>.
BINARY_RANGE = ["constructible", "trivially_constructible", "nothrow_constructible"]

# Operand restrictions of [meta.unary.prop] / [meta.rel] the zoo respects
# (a precondition violation is UB in <type_traits>, so it is not tested).
SIGN_OK = {"Int", "CInt", "VInt", "Bool", "Char", "UInt", "En", "SEn", "Long"}


def rows():
    """Yields (kiln expression, std expression) pairs."""
    for a, _ in UNARY:
        for t in UNARY_BOOL:
            yield f"is_{t}_type(^^{a})", f"std::is_{t}_v<{a}>"
        for r, s in UNARY_HAS:
            yield f"{r}(^^{a})", f"std::{s}_v<{a}>"
        for r, s in UNARY_SIZE:
            yield r.format(a), s.format(a)
    for a in BINARY:
        for b in BINARY:
            for t in BINARY_BOOL:
                yield f"is_{t}_type(^^{a}, ^^{b})", f"std::is_{t}_v<{a}, {b}>"
            for t in BINARY_PLAIN:
                yield f"{t}(^^{a}, ^^{b})", f"std::{t}_v<{a}, {b}>"
            for t in BINARY_RANGE:
                yield f"is_{t}_type(^^{a}, {{^^{b}}})", f"std::is_{t}_v<{a}, {b}>"


def transform_rows():
    for a, _ in UNARY:
        for t in TRANSFORMS:
            yield f"{t}(^^{a})", f"^^std::{t}_t<{a}>"
        if a in SIGN_OK and a != "Bool":
            for t in ("make_signed", "make_unsigned"):
                yield f"{t}(^^{a})", f"^^std::{t}_t<{a}>"
        if a in ("En", "SEn"):
            yield f"underlying_type(^^{a})", f"^^std::underlying_type_t<{a}>"


def aliases():
    return "".join(f"using {a} = {t};\n" for a, t in UNARY + EXTRA)


def probe(clang, pairs):
    src = ("#include <cstdio>\n#include <type_traits>\n#include <utility>\n" + PRELUDE
           + aliases() + "int main() {\n"
           + "".join(f'  std::printf("%llu\\n", (unsigned long long)({s}));\n' for _, s in pairs)
           + "}\n")
    with tempfile.TemporaryDirectory(prefix="kiln-zoo-") as d:
        cpp, exe = os.path.join(d, "probe.cpp"), os.path.join(d, "probe")
        with open(cpp, "w") as f:
            f.write(src)
        subprocess.run([clang, "-std=c++26", "-w", cpp, "-o", exe], check=True)
        out = subprocess.run([exe], check=True, capture_output=True, text=True).stdout.split()
    assert len(out) == len(pairs), (len(out), len(pairs))
    return out


HEADER = "// GENERATED by tests/conformance/meta.reflection.traits/gen/gen_zoo.py; do not edit.\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--clang", default="clang++-22")
    args = ap.parse_args()
    pairs = list(rows())
    values = probe(args.clang, pairs)
    zoo = (HEADER
           + "// [meta.reflection.traits] P2996R13: every type trait with a <type_traits>\n"
           + "// counterpart answers as it, over a type zoo (each type reflected through an\n"
           + "// alias: p4); a transformation returns the canonical type std::X_t names\n"
           + "// (p5). traits-zoo-rhs.cpp pins the std:: side to clang 22's answers.\n"
           + "//@ args: -freflection -I tests/support\n"
           + "//@ skip(clang): clang 19 does not implement P2996 reflection\n"
           + "//@ strict-diags\n"
           + "#include <type_traits>\n#include <utility>\n#include \"kiln_meta_traits.h\"\n"
           + "using namespace std::meta;\n" + PRELUDE + aliases()
           + "".join(f"static_assert({k} == {s});\n" for k, s in pairs)
           + "".join(f"static_assert({k} == {s});\n" for k, s in transform_rows()))
    rhs = (HEADER
           + "// [meta.unary], [meta.rel]: the std:: side of traits-zoo.cpp, each value\n"
           + "// clang 22's (computed when the file was generated), so kiln's <type_traits>\n"
           + "// answers and the oracle agree over the zoo.\n"
           + "//@ min-oracle: 22\n"
           + "//@ strict-diags\n"
           + "#include <type_traits>\n#include <utility>\n" + PRELUDE + aliases()
           + "".join(f"static_assert({s} == {v});\n" for (_, s), v in zip(pairs, values)))
    for name, text in (("traits-zoo.cpp", zoo), ("traits-zoo-rhs.cpp", rhs)):
        with open(os.path.join(OUT, name), "w") as f:
            f.write(text)
    print(f"{len(pairs)} trait rows, {sum(1 for _ in transform_rows())} transformation rows",
          file=sys.stderr)


if __name__ == "__main__":
    main()
