package aulo.chat.di

import app.cash.sqldelight.db.SqlDriver
import app.cash.sqldelight.driver.native.NativeSqliteDriver
import aulo.chat.data.cache.ChatDatabase
import org.koin.core.module.Module
import org.koin.dsl.module

fun iosPlatformModule(): Module = module {
    single<SqlDriver> { NativeSqliteDriver(ChatDatabase.Schema, DATABASE_NAME) }
}
