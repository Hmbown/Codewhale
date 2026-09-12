package codewhale.pet

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import kotlin.math.sin

/** Only places the worker's immutable dots. No clock, world or score here. */
@Composable
fun CodewhalePet(scene: PetScene, modifier: Modifier = Modifier) {
    Canvas(modifier = modifier.fillMaxSize().semantics { contentDescription = petContentDescription(scene) }) {
        val w = size.width; val h = size.height
        drawRect(Brush.verticalGradient(listOf(Color(0xff0b2029), Color(0xff071319))))
        val surfaceY = h * (1 + scene.surface) / 2
        val surface = Path().apply {
            moveTo(0f, surfaceY)
            for (i in 1..80) {
                val x = i * w / 80
                lineTo(x, surfaceY + sin(i * 0.10).toFloat() * h * 0.008f)
            }
        }
        drawPath(surface, Color(0xff709f9b), alpha = 0.22f, style = Stroke(1f))
        drawRect(Brush.verticalGradient(listOf(Color(0xff73c9b5).copy(alpha = scene.caustic * 0.025f), Color.Transparent)),
            topLeft = Offset(0f, surfaceY))
        drawLine(Color(0xff264048), Offset(0f, h * 0.93f), Offset(w, h * 0.93f), 1f)
        scene.food?.let { food ->
            drawCircle(Color(0xffc2b787), 3f, Offset(w * (0.5f + food.x * 0.3f), h * (0.5f + food.y * 0.3f)),
                alpha = food.life.coerceIn(0f, 1f))
        }
        val f = scene.style
        val color = Color(f.r.toInt().coerceIn(0, 255), f.g.toInt().coerceIn(0, 255), f.b.toInt().coerceIn(0, 255))
        val lay = petLayout(w.toDouble(), h.toDouble(), scene.state)
        val dot = lay.dot.toFloat()
        for (i in scene.dots.indices step 2) {
            val center = Offset((lay.ox + scene.dots[i] * lay.scale * lay.flipX).toFloat(),
                (lay.oy + scene.dots[i + 1] * lay.scale).toFloat())
            if (f.hollow) drawCircle(color, dot / 2, center, f.alpha.toFloat(), style = Stroke(1f))
            else drawCircle(color, dot / 2, center, f.alpha.toFloat())
        }
    }
}

fun petContentDescription(scene: PetScene): String = buildList {
    add("Codewhale"); add(scene.style.channel); add(scene.style.arch)
    if (scene.style.hollow) add("unobserved")
    add(scene.behaviour)
    if (scene.needs != "none" && scene.style.channel == "human" && !scene.style.hollow && scene.state.attention > 0.5)
        add("input pending")
    if (scene.peers >= 3) add("${scene.peers} pod members")
}.joinToString(", ")
