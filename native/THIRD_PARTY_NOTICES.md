# Upstream notices

## V8 reference algorithms and adapted test cases

The V9 scanner and arithmetic-expression work studies V8 revision
`fedd021b538607860f9c56e5c193ad8dcd4f2bb1`. The source map in
[UPSTREAM.md](UPSTREAM.md) distinguishes reviewed reference code from implemented
behavior. Retain this notice with source and binary distributions containing
adaptations.

Source file notices: Copyright 2011 the V8 project authors. All rights reserved.
The reviewed parser base carries Copyright 2012 the V8 project authors.
All rights reserved.
The additionally reviewed `src/objects/contexts.h` carries Copyright 2012 the V8
project authors; `src/objects/js-function.tq` carries Copyright 2020 the V8 project
authors. All rights reserved. The closure/context reference adaptations and owned
Rust graph collector are distinguished in UPSTREAM.md.
The additionally reviewed interpreter bytecode generator carries Copyright 2015
the V8 project authors. All rights reserved. The parser and interpreter routines
are semantically adapted to bounded Rust AST evaluation, as recorded in UPSTREAM.md.
The additionally reviewed `src/builtins/arm64/builtins-arm64.cc` carries Copyright
2013 the V8 project authors. All rights reserved. Receiver conversion is semantically
adapted to bounded Rust evaluation; its machine-code generator is not ported.
The additionally reviewed `src/builtins/function.tq`, `src/objects/js-function.tq`
and `src/builtins/conversion.tq` carry Copyright 2020 the V8 project authors;
`src/builtins/builtins-function.cc` carries Copyright 2016 the V8 project authors.
All rights reserved. The callable-object layer semantically adapts reviewed
algorithms rather than their Torque, native accessor or machine-code machinery.
The additionally reviewed `src/runtime/runtime-scopes.cc` carries Copyright 2014
the V8 project authors. All rights reserved. Global declaration instantiation is
semantically adapted to the owned bounded Rust binding graph, as recorded in UPSTREAM.md.
Adapted `test/mjsunit/number-tostring.js` cases: Copyright 2008 the V8 project
authors. All rights reserved.

The applicable redistribution terms from that revision's `LICENSE` follow:

Copyright 2014, the V8 project authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

* Redistributions of source code must retain the above copyright
  notice, this list of conditions and the following disclaimer.
* Redistributions in binary form must reproduce the above
  copyright notice, this list of conditions and the following
  disclaimer in the documentation and/or other materials provided
  with the distribution.
* Neither the name of Google Inc. nor the names of its
  contributors may be used to endorse or promote products derived
  from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## ryu-js

`ryu-js` 1.0.3 is a Rust numeric-formatting primitive, licensed under
Apache-2.0 OR BSL-1.0. It supplies ECMAScript number-to-string formatting,
not a parser, virtual machine or browser engine. Its license files ship with
the Cargo dependency. This package retains the selected Boost Software License
at [licenses/ryu-js/LICENSE-BOOST](licenses/ryu-js/LICENSE-BOOST).

## Image decoders

`png` 0.17.16 and `jpeg-decoder` 0.3.2 are licensed under MIT OR Apache-2.0.
Their license texts ship with the Cargo dependencies; the package uses their
bounded PNG and JPEG decoding APIs for static page images.

## macOS Keychain bindings

`security-framework` 3.7.0 and `security-framework-sys` 2.17.0 provide Rust
bindings to macOS Security.framework for the default desktop cookie profile.
They are licensed MIT OR Apache-2.0; their license texts ship with Cargo
dependencies.
