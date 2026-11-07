#!/bin/sh
# Compila y ejecuta los tests JVM de la lógica pura (paquete core) SIN Android SDK ni red, usando el
# compilador de Kotlin y JUnit 4 que trae una instalación local de Gradle (lib/).
# Uso: android/tools/test_core_offline.sh   (GRADLE_HOME=/opt/gradle-8.14.3 por defecto)
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
GH=${GRADLE_HOME:-$(dirname "$(dirname "$(readlink -f "$(command -v gradle)")")")}
L="$GH/lib"
KV=$(ls "$L" | sed -n 's/^kotlin-compiler-embeddable-\(.*\)\.jar$/\1/p' | head -n1)
[ -n "$KV" ] || { echo "No encuentro kotlin-compiler-embeddable en $L" >&2; exit 2; }
CP_KC="$L/kotlin-compiler-embeddable-$KV.jar:$L/kotlin-stdlib-$KV.jar:$L/kotlin-script-runtime-$KV.jar:$L/kotlin-reflect-$KV.jar:$L/kotlin-daemon-embeddable-$KV.jar"
for j in "$L"/trove4j-*.jar "$L"/annotations-*.jar "$L"/kotlinx-coroutines-core-jvm-*.jar; do CP_KC="$CP_KC:$j"; done
JUNIT=$(ls "$L"/junit-4*.jar | head -n1)
HAM=$(ls "$L"/hamcrest-core-*.jar | head -n1)
OUT=${TMPDIR:-/tmp}/jd-mando-core-tests
rm -rf "$OUT" && mkdir -p "$OUT"
SRC="$HERE/app/src"
java -cp "$CP_KC" org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -jvm-target 17 \
  -cp "$L/kotlin-stdlib-$KV.jar:$JUNIT" -d "$OUT" \
  "$SRC"/main/java/com/juegodebaile/mando/core/*.kt "$SRC"/test/java/com/juegodebaile/mando/core/*.kt
CLASSES=$(cd "$SRC/test/java" && ls com/juegodebaile/mando/core/*Test.kt | sed 's/\.kt$//; s#/#.#g')
java -cp "$OUT:$SRC/test/resources:$L/kotlin-stdlib-$KV.jar:$JUNIT:$HAM" org.junit.runner.JUnitCore $CLASSES
