package aulo.chat.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The platforms the UI kit has a page for and this app runs on. */
enum class UiPlatform { Android, Ios, MacOs, Windows, Linux }

expect fun currentUiPlatform(): UiPlatform

/** The colour roles the chat draws with; every platform fills them from its own kit page. */
@Immutable
data class ChatColors(
    val background: Color,
    val surface: Color,
    val raised: Color,
    val text: Color,
    val textMuted: Color,
    val accent: Color,
    val onAccent: Color,
    val userBubble: Color,
    val onUserBubble: Color,
    val danger: Color,
    val dangerContainer: Color,
    val onDangerContainer: Color,
    val warningContainer: Color,
    val onWarningContainer: Color,
    val infoContainer: Color,
    val onInfoContainer: Color,
    val outline: Color,
    val ok: Color,
    /** The conversation list beside the chat on a wide window. */
    val sidebar: Color,
    /** The open chat's row in that list. */
    val selection: Color,
)

@Immutable
data class ChatShapes(val bubble: Dp, val control: Dp, val chip: Dp)

@Immutable
data class PlatformTokens(val colors: ChatColors, val shapes: ChatShapes)

/**
 * The Dark and Light modes of each platform's colour collection in the Pyrlyn UIkit Figma file;
 * the names in the comments are the variable names there. Every value is written as
 * `pick(dark, light)` in ARGB. The kit's glass and tint colours are translucent; they are
 * flattened onto what they sit on, because the app draws no blur behind its surfaces. Radii
 * follow each platform's own convention.
 */
fun tokensFor(platform: UiPlatform, dark: Boolean = true): PlatformTokens {
    val p = Pick(dark)
    return when (platform) {
        UiPlatform.Android -> android(p)
        UiPlatform.Ios -> ios(p)
        UiPlatform.MacOs -> macOs(p)
        UiPlatform.Windows -> windows(p)
        UiPlatform.Linux -> linux(p)
    }
}

private class Pick(val dark: Boolean) {
    fun c(dark: Long, light: Long) = Color(if (this.dark) dark else light)

    fun over(dark: Long, light: Long, under: Color) = c(dark, light).compositeOver(under)
}

// Android · Color
private fun android(p: Pick) = PlatformTokens(
    ChatColors(
        background = p.c(0xFF121311, 0xFFF8F6F2), // surface
        surface = p.c(0xFF1F201D, 0xFFEDEBE7), // surface-container
        raised = p.c(0xFF292A27, 0xFFE7E5E1), // surface-container-high
        text = p.c(0xFFE4E2DE, 0xFF1B1C19), // on-surface
        textMuted = p.c(0xFFC6C6CF, 0xFF47474E), // on-surface-variant
        accent = p.c(0xFFA3C9FE, 0xFF335C8C), // primary
        onAccent = p.c(0xFF00315B, 0xFFFFFFFF), // on-primary
        userBubble = p.c(0xFF1B4876, 0xFFD3E3FF), // primary-container
        onUserBubble = p.c(0xFFD3E3FF, 0xFF0B1D33), // on-primary-container
        danger = p.c(0xFFFFB4AB, 0xFFBA1A1A), // error
        dangerContainer = p.c(0xFF93000A, 0xFFFFDAD6), // error-container
        onDangerContainer = p.c(0xFFFFDAD6, 0xFF410002), // on-error-container
        warningContainer = p.c(0xFF5E3A1C, 0xFFFFDCC4), // tertiary-container
        onWarningContainer = p.c(0xFFFFDCC4, 0xFF331200), // on-tertiary-container
        infoContainer = p.c(0xFF3C4758, 0xFFDCE3F1), // secondary-container
        onInfoContainer = p.c(0xFFD8E2F5, 0xFF141C29), // on-secondary-container
        outline = p.c(0xFF46464E, 0xFFC8C6CE), // outline-variant
        ok = p.c(0xFFC8F0D4, 0xFF0B3A20), // on-success-container
        sidebar = p.c(0xFF1C1D1A, 0xFFFFFFFF), // surface-container-lowest
        selection = p.c(0xFF3C4758, 0xFFDCE3F1), // secondary-container
    ),
    ChatShapes(bubble = 20.dp, control = 28.dp, chip = 8.dp),
)

// iOS · Color
private fun ios(p: Pick): PlatformTokens {
    val base = p.c(0xFF0B0F17, 0xFFEEF2F7) // bg/base
    return PlatformTokens(
        ChatColors(
            background = base,
            surface = p.over(0xC72C2C2E, 0xCCFFFFFF, base), // glass/fill-strong
            raised = p.over(0x942C2C2E, 0x9EFFFFFF, base), // glass/fill
            text = p.c(0xFFFFFFFF, 0xFF000000), // text/primary
            textMuted = p.over(0x99EBEBF5, 0x993C3C43, base), // text/secondary
            accent = p.c(0xFF7FA7DA, 0xFF335C8C), // accent
            onAccent = p.c(0xFFFFFFFF, 0xFFFFFFFF), // text/on-accent
            userBubble = p.c(0xFF4A78B0, 0xFF335C8C), // accent/fill
            onUserBubble = p.c(0xFFFFFFFF, 0xFFFFFFFF), // text/on-accent
            danger = p.c(0xFFFF8A80, 0xFFB42318), // red/text
            dangerContainer = p.over(0x33FF453A, 0x1FFF3B30, base), // red/bg
            onDangerContainer = p.c(0xFFFF8A80, 0xFFB42318), // red/text
            warningContainer = p.over(0x33F2A65A, 0x38F2A65A, base), // orange/bg
            onWarningContainer = p.c(0xFFF5B27A, 0xFFA4501C), // orange/text
            infoContainer = p.over(0x2E7FA7DA, 0x1F335C8C, base), // accent/soft
            onInfoContainer = p.c(0xFF7FA7DA, 0xFF335C8C), // accent
            // glass/stroke is white in both modes; on the light base it would vanish, so the
            // light mode takes fill/track.
            outline = if (p.dark) Color(0x24FFFFFF).compositeOver(base) else Color(0x24767680).compositeOver(base),
            ok = p.c(0xFF6FE29A, 0xFF1E7A4C), // green/text
            sidebar = base, // bg/base
            selection = p.over(0x2E7FA7DA, 0x1F335C8C, base), // accent/soft
        ),
        ChatShapes(bubble = 18.dp, control = 20.dp, chip = 12.dp),
    )
}

// Desktop · Color (the macOS kit page draws with it)
private fun macOs(p: Pick): PlatformTokens {
    val base = p.c(0xFF1B1D22, 0xFFF4F5F7) // bg/chrome
    return PlatformTokens(
        ChatColors(
            background = base,
            surface = p.c(0xFF1F1F24, 0xFFFFFFFF), // bg/editor
            raised = p.over(0x14FFFFFF, 0x0D000000, base), // bg/fill
            text = p.c(0xFFEEF2F8, 0xFF1D2129), // text/primary
            textMuted = p.c(0xFFA3ADBF, 0xFF5C6577), // text/secondary
            accent = p.c(0xFF6CB6FF, 0xFF0A64C2), // accent/ink
            onAccent = p.c(0xFF1B1D22, 0xFFFFFFFF), // bg/chrome, text/on-accent
            userBubble = p.c(0xFF0873E0, 0xFF0873E0), // accent/default
            onUserBubble = p.c(0xFFFFFFFF, 0xFFFFFFFF), // text/on-accent
            danger = p.c(0xFFFF8E9A, 0xFFD92D42), // status/error
            dangerContainer = p.c(0xFF3D1820, 0xFFFDE4E7), // banner/error-bg
            onDangerContainer = p.c(0xFFFFD5DA, 0xFF8A1626), // banner/error-ink
            warningContainer = p.c(0xFF3A2F12, 0xFFFFF4D6), // banner/warning-bg
            onWarningContainer = p.c(0xFFFFE6A3, 0xFF6B4A00), // banner/warning-ink
            infoContainer = p.c(0xFF0F2A44, 0xFFE3F0FF), // banner/info-bg
            onInfoContainer = p.c(0xFFCFE5FF, 0xFF0B3D73), // banner/info-ink
            outline = p.over(0x1FFFFFFF, 0x1F000000, base), // border/subtle
            ok = p.c(0xFF4FD98F, 0xFF1F9D5C), // status/ok
            sidebar = p.over(0x0DFFFFFF, 0x08000000, base), // bg/fill-subtle
            selection = p.over(0x3D0A84FF, 0x33007AFF, base), // bg/selection
        ),
        ChatShapes(bubble = 14.dp, control = 10.dp, chip = 8.dp),
    )
}

// Windows · Color
private fun windows(p: Pick): PlatformTokens {
    val base = p.c(0xFF202020, 0xFFF3F3F3) // surface/window-mica
    val dialog = p.c(0xFF2B2B2B, 0xFFFFFFFF) // surface/dialog
    return PlatformTokens(
        ChatColors(
            background = base,
            surface = dialog,
            raised = p.over(0x0DFFFFFF, 0xB2FFFFFF, dialog), // surface/card
            text = p.c(0xFFFFFFFF, 0xFF1B1B1B), // text/primary
            textMuted = p.c(0xFFC5C5C5, 0xFF5F5F5F), // text/secondary
            accent = p.c(0xFF6CB6FF, 0xFF0873E0), // accent/default
            onAccent = p.c(0xFF000000, 0xFFFFFFFF), // accent/on
            userBubble = p.c(0xFF6CB6FF, 0xFF0873E0), // accent/default
            onUserBubble = p.c(0xFF000000, 0xFFFFFFFF), // accent/on
            danger = p.c(0xFFFF99A4, 0xFFC42B1C), // status/critical
            dangerContainer = p.c(0xFF442726, 0xFFFDE7E9), // status/critical-bg
            onDangerContainer = p.c(0xFFFFFFFF, 0xFF1B1B1B), // text/primary
            warningContainer = p.c(0xFF433519, 0xFFFFF4CE), // status/caution-bg
            onWarningContainer = p.c(0xFFFFFFFF, 0xFF1B1B1B), // text/primary
            infoContainer = p.over(0x0DFFFFFF, 0xCCF6F6F6, base), // status/info-bg
            onInfoContainer = p.c(0xFFFFFFFF, 0xFF1B1B1B), // text/primary
            outline = p.over(0x12FFFFFF, 0x0F000000, base), // stroke/card
            ok = p.c(0xFF6CCB5F, 0xFF0F7B0F), // status/success
            sidebar = base, // surface/window-mica: the navigation pane sits on the window itself
            selection = p.over(0x15FFFFFF, 0x0F000000, base), // fill/subtle-selected
        ),
        ChatShapes(bubble = 8.dp, control = 4.dp, chip = 4.dp),
    )
}

// Linux · Color
private fun linux(p: Pick): PlatformTokens {
    val base = p.c(0xFF0A0C16, 0xFFEBEBED) // color/glass-bar
    val panel = p.c(0xFF1E2132, 0xFFFFFFFF) // color/glass-strong
    return PlatformTokens(
        ChatColors(
            background = base,
            surface = panel,
            raised = p.over(0x1FFFFFFF, 0x14000006, panel), // color/fill-2
            text = p.over(0xFFF1F4FA, 0xCC000006, base), // color/ink
            textMuted = p.over(0xBDECF1FA, 0x99000006, base), // color/ink-muted
            // The mint fill reads poorly as text on the light base; the light mode's accent is
            // color/accent-text, the kit's own text tone of it.
            accent = p.c(0xFF3DDCB0, 0xFF00785A), // color/accent, color/accent-text
            onAccent = p.c(0xFF06261F, 0xFFFFFFFF), // color/accent-ink, color/on-tone
            userBubble = p.c(0xFF3DDCB0, 0xFF3DDCB0), // color/accent
            onUserBubble = p.c(0xFF06261F, 0xFF06261F), // color/accent-ink
            danger = p.c(0xFFFF9A92, 0xFFC30000), // color/danger-text
            dangerContainer = p.over(0x24FF9A92, 0x1AE01B24, base), // color/danger-tint
            onDangerContainer = p.c(0xFFFF9A92, 0xFFC30000), // color/danger-text
            warningContainer = p.over(0x29FFC06B, 0x29E5A50A, base), // color/warning at the tint alpha
            onWarningContainer = p.over(0xFFFFC06B, 0xCC000000, base), // color/warning, color/warning-ink
            infoContainer = p.over(0x296FB1FF, 0x292F6FDE, base), // color/tone/blue at the tint alpha
            onInfoContainer = p.c(0xFFA9CEFF, 0xFF2F6FDE), // color/tone/blue
            outline = p.over(0x21FFFFFF, 0x1F000006, base), // color/glass-line
            ok = p.c(0xFF2FBF93, 0xFF178A69), // color/badge-ok, color/badge-done
            sidebar = base, // color/glass-bar
            selection = p.over(0x29FFFFFF, 0x1A000006, base), // color/selected-gradient/start
        ),
        ChatShapes(bubble = 12.dp, control = 9.dp, chip = 6.dp),
    )
}
