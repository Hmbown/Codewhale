package codewhale.pet

import java.io.File
import kotlin.math.floor
import kotlin.math.min
import kotlin.math.roundToInt

fun petDigest(sim: PetSim): String {
    val grid = IntArray(64 * 32)
    for (p in sim.p) {
        val x = floor((p.x + 0.66) / 1.32 * 64).toInt()
        val y = floor((p.y + 0.66) / 1.32 * 32).toInt()
        if (x in 0..63 && y in 0..31) grid[y * 64 + x] = min(255, grid[y * 64 + x] + 1)
    }
    var hash = 0xcbf29ce484222325UL.toLong()
    fun mix(n: Int) { hash = (hash xor (n and 255).toLong()) * 0x100000001b3L }
    for (n in grid) mix(n)
    mix(sim.frame.r.roundToInt()); mix(sim.frame.g.roundToInt()); mix(sim.frame.b.roundToInt())
    mix((sim.frame.alpha * 255).roundToInt()); mix(if (sim.frame.hollow) 1 else 0)
    return hash.toULong().toString(16).padStart(16, '0')
}

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
