package aulo.chat.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The platforms the UI kit has a page for and this app runs on. */
enum class UiPlatform { Android, Ios, MacOs, Windows, Linux }

expect fun currentUiPlatform(): UiPlatform

/** The colour roles the chat screen draws with; every platform fills them from its own kit page. */
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
)

@Immutable
data class ChatShapes(val bubble: Dp, val control: Dp, val chip: Dp)

@Immutable
data class PlatformTokens(val colors: ChatColors, val shapes: ChatShapes)

/**
 * The Dark mode of each platform's colour collection in the Pyrlyn UIkit Figma file; the names
 * in the comments are the variable names there. The kit's glass and tint colours are
 * translucent; they are flattened onto the background here because the app draws no blur
 * behind its surfaces. Radii follow each platform's own convention.
 */
fun tokensFor(platform: UiPlatform): PlatformTokens = when (platform) {
    UiPlatform.Android -> PlatformTokens(
        // Android · Color
        ChatColors(
            background = Color(0xFF121311), // surface
            surface = Color(0xFF1F201D), // surface-container
            raised = Color(0xFF292A27), // surface-container-high
            text = Color(0xFFE4E2DE), // on-surface
            textMuted = Color(0xFFC6C6CF), // on-surface-variant
            accent = Color(0xFFA3C9FE), // primary
            onAccent = Color(0xFF00315B), // on-primary
            userBubble = Color(0xFF1B4876), // primary-container
            onUserBubble = Color(0xFFD3E3FF), // on-primary-container
            danger = Color(0xFFFFB4AB), // error
            dangerContainer = Color(0xFF93000A), // error-container
            onDangerContainer = Color(0xFFFFDAD6), // on-error-container
            warningContainer = Color(0xFF5E3A1C), // tertiary-container
            onWarningContainer = Color(0xFFFFDCC4), // on-tertiary-container
            infoContainer = Color(0xFF3C4758), // secondary-container
            onInfoContainer = Color(0xFFD8E2F5), // on-secondary-container
            outline = Color(0xFF46464E), // outline-variant
            ok = Color(0xFFC8F0D4), // on-success-container
        ),
        ChatShapes(bubble = 20.dp, control = 28.dp, chip = 8.dp),
    )

    UiPlatform.Ios -> Color(0xFF0B0F17).let { base -> // iOS · Color, bg/base
        PlatformTokens(
            ChatColors(
                background = base,
                surface = Color(0xC72C2C2E).compositeOver(base), // glass/fill-strong
                raised = Color(0x942C2C2E).compositeOver(base), // glass/fill
                text = Color(0xFFFFFFFF), // text/primary
                textMuted = Color(0x99EBEBF5).compositeOver(base), // text/secondary
                accent = Color(0xFF7FA7DA), // accent
                onAccent = Color(0xFFFFFFFF), // text/on-accent
                userBubble = Color(0xFF4A78B0), // accent/fill
                onUserBubble = Color(0xFFFFFFFF), // text/on-accent
                danger = Color(0xFFFF8A80), // red/text
                dangerContainer = Color(0x33FF453A).compositeOver(base), // red/bg
                onDangerContainer = Color(0xFFFF8A80), // red/text
                warningContainer = Color(0x33F2A65A).compositeOver(base), // orange/bg
                onWarningContainer = Color(0xFFF5B27A), // orange/text
                infoContainer = Color(0x2E7FA7DA).compositeOver(base), // accent/soft
                onInfoContainer = Color(0xFF7FA7DA), // accent
                outline = Color(0x24FFFFFF).compositeOver(base), // glass/stroke
                ok = Color(0xFF6FE29A), // green/text
            ),
            ChatShapes(bubble = 18.dp, control = 20.dp, chip = 12.dp),
        )
    }

    UiPlatform.MacOs -> Color(0xFF1B1D22).let { base -> // macOS · Color, bg/chrome
        PlatformTokens(
            ChatColors(
                background = base,
                surface = Color(0xFF1F1F24), // bg/editor
                raised = Color(0x14FFFFFF).compositeOver(base), // bg/fill
                text = Color(0xFFEEF2F8), // text/primary
                textMuted = Color(0xFFA3ADBF), // text/secondary
                accent = Color(0xFF6CB6FF), // accent/ink
                onAccent = Color(0xFF1B1D22), // bg/chrome
                userBubble = Color(0xFF0873E0), // accent/default
                onUserBubble = Color(0xFFFFFFFF), // text/on-accent
                danger = Color(0xFFFF8E9A), // status/error
                dangerContainer = Color(0xFF3D1820), // banner/error-bg
                onDangerContainer = Color(0xFFFFD5DA), // banner/error-ink
                warningContainer = Color(0xFF3A2F12), // banner/warning-bg
                onWarningContainer = Color(0xFFFFE6A3), // banner/warning-ink
                infoContainer = Color(0xFF0F2A44), // banner/info-bg
                onInfoContainer = Color(0xFFCFE5FF), // banner/info-ink
                outline = Color(0x1FFFFFFF).compositeOver(base), // border/subtle
                ok = Color(0xFF4FD98F), // status/ok
            ),
            ChatShapes(bubble = 14.dp, control = 10.dp, chip = 8.dp),
        )
    }

    UiPlatform.Windows -> Color(0xFF202020).let { base -> // Windows · Color, surface/window-mica
        PlatformTokens(
            ChatColors(
                background = base,
                surface = Color(0xFF2B2B2B), // surface/dialog
                raised = Color(0x0DFFFFFF).compositeOver(Color(0xFF2B2B2B)), // surface/card
                text = Color(0xFFFFFFFF), // text/primary
                textMuted = Color(0xFFC5C5C5), // text/secondary
                accent = Color(0xFF6CB6FF), // accent/default
                onAccent = Color(0xFF000000), // accent/on
                userBubble = Color(0xFF6CB6FF), // accent/default
                onUserBubble = Color(0xFF000000), // accent/on
                danger = Color(0xFFFF99A4), // status/critical
                dangerContainer = Color(0xFF442726), // status/critical-bg
                onDangerContainer = Color(0xFFFFFFFF), // text/primary
                warningContainer = Color(0xFF433519), // status/caution-bg
                onWarningContainer = Color(0xFFFFFFFF), // text/primary
                infoContainer = Color(0x0DFFFFFF).compositeOver(base), // status/info-bg
                onInfoContainer = Color(0xFFFFFFFF), // text/primary
                outline = Color(0x12FFFFFF).compositeOver(base), // stroke/card
                ok = Color(0xFF6CCB5F), // status/success
            ),
            ChatShapes(bubble = 8.dp, control = 4.dp, chip = 4.dp),
        )
    }

    UiPlatform.Linux -> Color(0xFF0A0C16).let { base -> // Linux · Color, color/glass-bar
        val panel = Color(0xFF1E2132) // color/glass-strong
        PlatformTokens(
            ChatColors(
                background = base,
                surface = panel,
                raised = Color(0x1FFFFFFF).compositeOver(panel), // color/fill-2
                text = Color(0xFFF1F4FA), // color/ink
                textMuted = Color(0xBDECF1FA).compositeOver(base), // color/ink-muted
                accent = Color(0xFF3DDCB0), // color/accent
                onAccent = Color(0xFF06261F), // color/accent-ink
                userBubble = Color(0xFF3DDCB0), // color/accent
                onUserBubble = Color(0xFF06261F), // color/accent-ink
                danger = Color(0xFFFF9A92), // color/danger-text
                dangerContainer = Color(0x29FF9A92).compositeOver(base), // status/danger-bg
                onDangerContainer = Color(0xFFFF9A92), // color/danger-text
                warningContainer = Color(0x29FFC06B).compositeOver(base), // status/warning-bg
                onWarningContainer = Color(0xFFFFC06B), // color/warning
                infoContainer = Color(0x296FB1FF).compositeOver(base), // status/info-bg
                onInfoContainer = Color(0xFFA9CEFF), // status/info-text
                outline = Color(0x21FFFFFF).compositeOver(base), // color/glass-line
                ok = Color(0xFF2FBF93), // color/badge-ok
            ),
            ChatShapes(bubble = 12.dp, control = 9.dp, chip = 6.dp),
        )
    }
}
