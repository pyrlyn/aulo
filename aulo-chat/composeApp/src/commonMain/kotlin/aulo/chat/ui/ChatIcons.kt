package aulo.chat.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

/**
 * The handful of glyphs the chat screen needs, as 24 dp stroke paths. They are drawn here
 * because the Compose Material icon artifacts stopped at 1.7.3 and are no longer maintained.
 */
internal object ChatIcons {
    val Send = stroke("Send", "M12 19V5M5.5 11.5L12 5l6.5 6.5")
    val Stop = filled("Stop", "M8 7h8a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1z")
    val Add = stroke("Add", "M12 5v14M5 12h14")
    val Close = stroke("Close", "M6.5 6.5l11 11M17.5 6.5l-11 11")
    val ChevronDown = stroke("ChevronDown", "M6 9.5l6 6 6-6")
    val Check = stroke("Check", "M5 12.5l4.5 4.5L19 7.5")
    val Document = stroke("Document", "M7 3h7l4 4v14H7zM14 3v4h4M10 12h5M10 16h5")
    val Image = stroke("Image", "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M15.5 9.5h.01")
    val Mic = stroke("Mic", "M12 4a3 3 0 0 1 3 3v4a3 3 0 0 1-6 0V7a3 3 0 0 1 3-3zM6 11a6 6 0 0 0 12 0M12 17v3")

    private fun stroke(name: String, path: String): ImageVector = builder(name).addPath(
        pathData = addPathNodes(path),
        stroke = SolidColor(Color.Black),
        strokeLineWidth = 2f,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    ).build()

    private fun filled(name: String, path: String): ImageVector =
        builder(name).addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black)).build()

    private fun builder(name: String) = ImageVector.Builder(
        name = name,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    )
}
