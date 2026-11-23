package aulo.chat.ui

import androidx.compose.ui.window.ComposeUIViewController
import aulo.chat.data.ServerEndpoint
import aulo.chat.di.chatModules
import aulo.chat.di.iosPlatformModule
import org.koin.core.context.startKoin
import platform.Foundation.NSBundle
import platform.UIKit.UIViewController

// Started once, however many times SwiftUI asks for the controller.
private val koin by lazy {
    startKoin { modules(chatModules(endpointFromBundle(), iosPlatformModule())) }
}

/**
 * The server the app was built for: the `AuloChatHost` and `AuloChatPort` keys of Info.plist,
 * which the Xcode project fills from build settings. Left empty they give the loopback
 * default, which on the simulator is the Mac itself.
 */
private fun endpointFromBundle(): ServerEndpoint {
    val bundle = NSBundle.mainBundle
    val host = bundle.objectForInfoDictionaryKey("AuloChatHost") as? String
    val port = bundle.objectForInfoDictionaryKey("AuloChatPort") as? String
    return ServerEndpoint.parse(host, port) ?: error("AuloChatPort in Info.plist must be a port number, got \"$port\"")
}

/** The entry point the Swift wrapper calls: `MainViewControllerKt.MainViewController()`. */
@Suppress("FunctionName")
fun MainViewController(): UIViewController {
    koin
    val version = NSBundle.mainBundle.objectForInfoDictionaryKey("CFBundleShortVersionString") as? String
    return ComposeUIViewController { App(version.orEmpty()) }
}
