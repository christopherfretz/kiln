// The declaration-extent table (spec/ast.md §19.6, §19.9): module units
// replayed from their sources and a header unit built from its source
// record their rows by parsing, the same rows module-bodies-image.cpp's
// importer builds from the images. This file keeps that file's lines,
// so the two share their golden (the imports and the use are on the
// same lines); both compare it as a set of rows, since the harness's
// `--bmi-image` lane builds these modules as images too. Every row of a declaration inside a body
// comes from an inline function, an inline variable's initializer or a
// template's pattern, the definitions an importer reaches
// ([module.reach]/1); a body the image does not carry (a non-inline
// function's) is not compared. The module declarations and the import
// declarations of every unit are rows too: the importer of an image
// with a preamble preprocesses it, as replay does, and an image without
// one carries its module declaration's row.
//
//@ module: extents_body:part=Inputs/extents_body_part.cppm
//@ module: extents_body=Inputs/extents_body.cppm
//@ header-unit: "Inputs/extents_hu.h"
//@ decl-extents: module-bodies.extents unordered
//@ args: -fkiln-decl-extents -Wno-experimental-header-units
//@ std: c++20
//
import extents_body [[]];
import "Inputs/extents_hu.h";
int use() { return body(1) + pattern(2) + split(3) + initialized + from_header(4); }
