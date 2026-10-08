package codewhale.pet

import java.io.File

fun main(args: Array<String>) {
    require(args.isNotEmpty()) { "Usage: points.tsv [--reduced-motion]; tape on stdin" }
    val points = File(args[0]).readLines().filter { it.isNotBlank() }.map { row ->
        val c = row.split('\t').map { it.toDouble() }; require(c.size == 2); Pair(c[0], c[1])
    }
    val sim = PetSim(points, expressionVersion = if (args.contains("--legacy")) 1 else 2); var frame = 0
    generateSequence(::readlnOrNull).forEach { row ->
        val c = row.split('\t')
        if (c.size >= 10 && c[0] != "dt") {
            val state = PetState(c[1].toDouble(), c[2].toDouble(), c[3].toDouble(), c[4], c[5].toDouble(),
                c[6].toDouble(), c[7].toDouble(), c[8].toDouble(), c[9].toDouble())
            sim.step(c[0].toDouble(), state, !args.contains("--reduced-motion"))
            if (frame % 30 == 0) println("f${frame.toString().padStart(4, '0')} ${petDigest(sim)} ${state.channel}")
            frame++
        }
    }
    println("final ${petDigest(sim)}")
}
