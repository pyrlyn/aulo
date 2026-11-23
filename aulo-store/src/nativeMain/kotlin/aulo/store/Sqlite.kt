package aulo.store

import db.DbDriver

/** The SQLite this platform links: kotlin-kit's db-room on macOS and Linux, db-native on Windows. */
internal expect val sqlite: DbDriver
