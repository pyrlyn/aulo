package aulo.store

import db.DbDriver
import db.room.RoomDbDriver

internal actual val sqlite: DbDriver = RoomDbDriver
