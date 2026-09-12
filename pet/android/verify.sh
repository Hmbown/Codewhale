#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# Use a pinned Kotlin installation, or an explicitly selected cached Gradle library.
if command -v kotlinc >/dev/null 2>&1; then
    kotlinc android/PetSim.kt android/Conformance.kt -include-runtime -d android/conformance.jar
    kotlin_classpath=android/conformance.jar
elif [ -n "${PET_KOTLIN_LIB:-}" ]; then
    java -cp "$PET_KOTLIN_LIB/*" org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect -classpath "$PET_KOTLIN_LIB/kotlin-stdlib-2.3.0.jar" -d android/conformance.jar android/PetSim.kt android/Conformance.kt
    kotlin_classpath="android/conformance.jar:$PET_KOTLIN_LIB/kotlin-stdlib-2.3.0.jar"
else
    printf '%s\n' 'Kotlin compiler missing: install Kotlin 2.3.0 or set PET_KOTLIN_LIB to its cached Gradle lib directory.' >&2
    exit 1
fi
for tape in tape.tsv tapes/edge-cases.tsv; do
    case "$tape" in tape.tsv) name=baseline ;; *) name=edge-cases ;; esac
    for mode in animated reduced-motion; do
        set --
        if [ "$mode" = reduced-motion ]; then set -- --reduced-motion; fi
        java -cp "$kotlin_classpath" codewhale.pet.ConformanceKt whale-points.tsv "$@" < "$tape" > "conformance-results/$name-$mode-kotlin.txt"
        diff -u "conformance-results/$name-$mode-ts.txt" "conformance-results/$name-$mode-kotlin.txt"
        printf 'PASS Kotlin %s %s\n' "$name" "$mode"
    done
done
