package aulo.chat.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf

val LocalChatColors = staticCompositionLocalOf<ChatColors> { error("AuloTheme is missing") }
val LocalChatShapes = staticCompositionLocalOf<ChatShapes> { error("AuloTheme is missing") }

/**
 * Material 3 dressed in the platform's tokens. The app has one theme, the dark one: it is the
 * mode every screen of the mockups is drawn in.
 */
@Composable
fun AuloTheme(platform: UiPlatform = remember { currentUiPlatform() }, content: @Composable () -> Unit) {
    val tokens = remember(platform) { tokensFor(platform) }
    val c = tokens.colors
    val scheme = darkColorScheme(
        primary = c.accent,
        onPrimary = c.onAccent,
        primaryContainer = c.userBubble,
        onPrimaryContainer = c.onUserBubble,
        secondaryContainer = c.infoContainer,
        onSecondaryContainer = c.onInfoContainer,
        background = c.background,
        onBackground = c.text,
        surface = c.surface,
        onSurface = c.text,
        surfaceVariant = c.raised,
        onSurfaceVariant = c.textMuted,
        surfaceContainerLowest = c.background,
        surfaceContainerLow = c.surface,
        surfaceContainer = c.surface,
        surfaceContainerHigh = c.raised,
        surfaceContainerHighest = c.raised,
        error = c.danger,
        errorContainer = c.dangerContainer,
        onErrorContainer = c.onDangerContainer,
        outline = c.outline,
        outlineVariant = c.outline,
    )
    val shapes = Shapes(
        extraSmall = RoundedCornerShape(tokens.shapes.chip),
        small = RoundedCornerShape(tokens.shapes.chip),
        medium = RoundedCornerShape(tokens.shapes.control),
        large = RoundedCornerShape(tokens.shapes.bubble),
        extraLarge = RoundedCornerShape(tokens.shapes.bubble),
    )
    CompositionLocalProvider(LocalChatColors provides c, LocalChatShapes provides tokens.shapes) {
        MaterialTheme(colorScheme = scheme, shapes = shapes, content = content)
    }
}
