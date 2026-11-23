package aulo.server

import aulo.store.Store
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * The daemon's one [Store], shared by every service. A `Store` is not thread-safe, so calls take
 * turns; SQLite blocks, so they run off the transport's threads. Closing the store stays with
 * whoever opened it.
 */
class SharedStore(
    private val store: Store,
) {
    private val mutex = Mutex()

    /** Runs [block] alone on the store; several operations in one block see no other writer in between. */
    suspend fun <T> use(block: Store.() -> T): T = mutex.withLock { withContext(Dispatchers.IO) { store.block() } }
}
