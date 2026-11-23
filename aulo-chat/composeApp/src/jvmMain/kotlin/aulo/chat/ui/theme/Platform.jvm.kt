package aulo.chat.ui.theme

actual fun currentUiPlatform(): UiPlatform {
    val os = System.getProperty("os.name").orEmpty().lowercase()
    return when {
        "mac" in os -> UiPlatform.MacOs
        "win" in os -> UiPlatform.Windows
        else -> UiPlatform.Linux
    }
}
