package aulo.chat.di

import android.content.Context
import app.cash.sqldelight.db.SqlDriver
import app.cash.sqldelight.driver.android.AndroidSqliteDriver
import aulo.chat.data.cache.ChatDatabase
import org.koin.core.module.Module
import org.koin.dsl.module

fun androidPlatformModule(context: Context): Module = module {
    single<SqlDriver> { AndroidSqliteDriver(ChatDatabase.Schema, context.applicationContext, DATABASE_NAME) }
}
