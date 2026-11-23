package aulo.chat.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import aulo.chat.domain.Appearance

val LocalChatColors = staticCompositionLocalOf<ChatColors> { error("AuloTheme is missing") }
val LocalChatShapes = staticCompositionLocalOf<ChatShapes> { error("AuloTheme is missing") }
val LocalUiPlatform = staticCompositionLocalOf { UiPlatform.Android }

/** Dark or light for the user's choice in Settings; System follows the operating system. */
@Composable
fun Appearance.isDark(): Boolean = when (this) {
    Appearance.System -> isSystemInDarkTheme()
    Appearance.Light -> false
    Appearance.Dark -> true
}

/** Material 3 dressed in the platform's tokens, in the dark or the light mode of its kit page. */
@Composable
fun AuloTheme(
    platform: UiPlatform = remember { currentUiPlatform() },
    dark: Boolean = true,
    content: @Composable () -> Unit,
) {
    val tokens = remember(platform, dark) { tokensFor(platform, dark) }
    val c = tokens.colors
    // Roles the tokens do not name keep Material's defaults for the mode.
    val scheme = (if (dark) darkColorScheme() else lightColorScheme()).copy(
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
    CompositionLocalProvider(
        LocalChatColors provides c,
        LocalChatShapes provides tokens.shapes,
        LocalUiPlatform provides platform,
    ) {
        MaterialTheme(colorScheme = scheme, shapes = shapes, content = content)
    }
}
