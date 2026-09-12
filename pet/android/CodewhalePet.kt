// CodewhalePet.kt — the Jetpack Compose renderer for the pet.
//
// The composable owns no simulation: the caller steps `sim` (e.g. from a
// `withFrameNanos` loop in a LaunchedEffect) and passes the `PetState` that
// produced the frame — the same contract as the ratatui widget and the
// SwiftUI view. One Canvas pass:
//
//   * filled circle per particle, colour and alpha from sim.frame
//   * hollow frames stroke the circles instead of filling them
//   * the caption row carries the non-colour cue ("tool · strike · unobserved")
//
// Reduced motion: honour the system's animator-duration-scale / accessibility
// setting by stepping the sim with motion = false — same contract as every
// other port. TalkBack gets petContentDescription(), not "a whale animation".
//
// NOT COMPILE-VERIFIED: the Kotlin core passes offline conformance, but the
// required Compose dependencies are not available in the inspected local cache.

package codewhale.pet

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics

@Composable
fun CodewhalePet(sim: PetSim, state: PetState, modifier: Modifier = Modifier) {
    val f = sim.frame
    val color = Color(f.r.toInt() and 0xff, f.g.toInt() and 0xff, f.b.toInt() and 0xff)
    Column(modifier = modifier.semantics { contentDescription = petContentDescription(sim, state) }) {
        Canvas(modifier = Modifier.weight(1f).fillMaxSize()) {
            val lay = petLayout(size.width.toDouble(), size.height.toDouble(), state)
            val d = lay.dot.toFloat()
            for (q in sim.p) {
                val px = (lay.ox + q.x * lay.scale * lay.flipX).toFloat()
                val py = (lay.oy + q.y * lay.scale).toFloat()
                if (f.hollow) {
                    drawCircle(color, radius = d / 2, center = Offset(px, py),
                        alpha = f.alpha.toFloat(), style = Stroke(width = 1f))
                } else {
                    drawCircle(color, radius = d / 2, center = Offset(px, py),
                        alpha = f.alpha.toFloat())
                }
            }
        }
        Text(
            "${f.channel} · ${f.arch}${if (f.hollow) " · unobserved" else ""}",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

fun petContentDescription(sim: PetSim, state: PetState): String {
    val f = sim.frame
    return buildList {
        add("Codewhale pet"); add(f.channel); add(f.arch)
        if (f.hollow) add("unobserved")
        if (state.lit < 0.5) add("dozing")
        if (state.attention > 0.5) add("needs you")
    }.joinToString(", ")
}
