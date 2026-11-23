package aulo.chat.di

import app.cash.sqldelight.db.SqlDriver
import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import aulo.chat.data.cache.ChatDatabase
import org.koin.core.module.Module
import org.koin.dsl.module
import java.io.File
import java.util.Properties

/** [dataDir] is where the cache file goes; null keeps it in memory, which tests use. */
fun desktopPlatformModule(dataDir: File?): Module = module {
    single<SqlDriver> {
        val url = if (dataDir == null) JdbcSqliteDriver.IN_MEMORY else {
            dataDir.mkdirs()
            "jdbc:sqlite:" + File(dataDir, DATABASE_NAME).absolutePath
        }
        JdbcSqliteDriver(url, Properties(), ChatDatabase.Schema)
    }
}
