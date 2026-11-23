package aulo.chat.di

import aulo.chat.data.ChatCache
import aulo.chat.data.ChatRemote
import aulo.chat.data.DefaultChatRepository
import aulo.chat.data.GrpcChatRemote
import aulo.chat.data.SampleAttachmentPicker
import aulo.chat.data.ServerEndpoint
import aulo.chat.domain.AttachmentPicker
import aulo.chat.domain.ChatRepository
import aulo.chat.domain.LoadModels
import aulo.chat.domain.OpenChat
import aulo.chat.domain.SendMessage
import aulo.chat.domain.StopGeneration
import aulo.chat.domain.SwitchModel
import aulo.chat.presentation.ChatViewModel
import org.koin.core.module.Module
import org.koin.core.module.dsl.viewModelOf
import org.koin.dsl.module

/**
 * The object graph of the chat. The platform module passed in supplies the one thing that
 * differs per platform: the SQLite driver the cache opens.
 */
fun chatModules(endpoint: ServerEndpoint, platform: Module): List<Module> = listOf(
    platform,
    module {
        single { endpoint }
        single<ChatRemote> { GrpcChatRemote(get()) }
        single { ChatCache(get()) }
        single<ChatRepository> { DefaultChatRepository(get(), get()) }
        single<AttachmentPicker> { SampleAttachmentPicker() }
        factory { OpenChat(get()) }
        factory { LoadModels(get()) }
        factory { SendMessage() }
        factory { StopGeneration() }
        factory { SwitchModel() }
        viewModelOf(::ChatViewModel)
    },
)

internal const val DATABASE_NAME = "aulo-chat.db"
