#!/bin/sh
# Recompile the test fixtures (needs a JDK 17+). The stubs under java/stubs are minimal
# stand-ins written for these tests; only the plugin classes are kept as fixtures.
set -e
cd "$(dirname "$0")"
rm -rf build classes
mkdir -p build classes
javac --release 17 -nowarn -XDignore.symbol.file -d build $(find java -name '*.java')
(cd build && find com -name '*.class' | while read -r c; do mkdir -p "../classes/$(dirname "$c")"; cp "$c" "../classes/$c"; done)
rm -rf build
