package aulo.chat.android

import android.app.Application
import aulo.chat.data.ServerEndpoint
import aulo.chat.di.androidPlatformModule
import aulo.chat.di.chatModules
import org.koin.core.context.startKoin

class ChatApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        val endpoint = ServerEndpoint(BuildConfig.SERVER_HOST, BuildConfig.SERVER_PORT)
        startKoin { modules(chatModules(endpoint, androidPlatformModule(this@ChatApplication))) }
    }
}
